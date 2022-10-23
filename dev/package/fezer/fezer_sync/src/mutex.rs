/*

    非同期ロック方式のMutex

    ----------------------------------------------------------------------------

    # 概要

    通常のMutexはロックを獲得中に `.await` を挟むことはできない。fezer_syncの
    Mutexを用いることでこの性質を回避することができるが、通常は推奨されない。
    `.await` を挟まない処理であれば通常のMutexを使用することに問題はない。

    獲得できるガードはRAIIスコープのロックであり、ドロップされルト期に自動で解除
    される。

    ```rust
    let val = 0;
    let val_mutex = Mutex::new(val);

    //  Bad
    {
        let mut lock = val_mutex.lock().await;
        *lock += 1;

        do_something_async().await;
    }

    //  Good
    {
        let mut lock = val_mutex.lock().await;
        *lock += 1;
    }
    do_something_async().await;
    ```

    ガードを保持している間にタスクがパニックに陥ると、MutexはPoisonedになり、そ
    の後のロックの呼び出しはパニックになる。

*/

use core::future::Future;
use core::ops::{ Deref, DerefMut };
use core::pin::Pin;
use core::task::{ Context, Poll };
use std::collections::VecDeque;
use std::task::Waker;
use std::sync::TryLockError;

//------------------------------------------------------------------------------
//  MutexGuard
//------------------------------------------------------------------------------
pub struct MutexGuard<'a, T>
{
    mutex: &'a Mutex<T>,
    value_guard: Option<std::sync::MutexGuard<'a, T>>,
}

impl<'a, T> MutexGuard<'a, T>
{
    //--------------------------------------------------------------------------
    //  新しいMutexガードを獲得
    //--------------------------------------------------------------------------
    pub(crate) fn new( mutex: &'a Mutex<T>, value_guard: std::sync::MutexGuard<'a, T> )
        -> MutexGuard<'a, T>
    {
        let mut inner_guard = mutex.inner.lock().unwrap();
        assert!(inner_guard.locked == false);
        inner_guard.locked = true;
        MutexGuard
        {
            mutex,
            value_guard: Some(value_guard),
        }
    }
}

impl<'a, T> Drop for MutexGuard<'a, T>
{
    //--------------------------------------------------------------------------
    //  drop
    //--------------------------------------------------------------------------
    fn drop( &mut self )
    {
        let mut wakers = VecDeque::new();

        {
            let mut inner_guard = self.mutex.inner.lock().unwrap();
            assert!(inner_guard.locked == true);
            inner_guard.locked = false;
            std::mem::swap(&mut inner_guard.wakers, &mut wakers);
        }

        self.value_guard.take();
        for waker in wakers
        {
            waker.wake();
        }
    }
}

impl<'a, T> Deref for MutexGuard<'a, T>
{
    type Target = T;

    //--------------------------------------------------------------------------
    //  deref
    //--------------------------------------------------------------------------
    fn deref( &self ) -> &Self::Target
    {
        &*self.value_guard.as_ref().unwrap()
    }
}

impl<'a, T> DerefMut for MutexGuard<'a, T>
{
    //--------------------------------------------------------------------------
    //  deref_mut
    //--------------------------------------------------------------------------
    fn deref_mut( &mut self ) -> &mut Self::Target
    {
        &mut *self.value_guard.as_mut().unwrap()
    }
}

//------------------------------------------------------------------------------
//  LockFuture
//------------------------------------------------------------------------------
pub struct LockFuture<'a, T>
{
    pub(crate) mutex: &'a Mutex<T>,
}

impl<'a, T> Future for LockFuture<'a, T>
{
    type Output = MutexGuard<'a, T>;

    //--------------------------------------------------------------------------
    //  poll
    //--------------------------------------------------------------------------
    fn poll( self: Pin<&mut Self>, cx: &mut Context<'_> ) -> Poll<Self::Output>
    {
        loop
        {
            //  ロックを獲得
            match self.mutex.value.try_lock()
            {
                Ok(guard) => return Poll::Ready(MutexGuard::new(self.mutex, guard)),
                Err(TryLockError::Poisoned(e)) => panic!("{}", e),
                Err(TryLockError::WouldBlock) => {},
            }

            let mut guard = self.mutex.inner.lock().unwrap();
            if guard.locked == true
            {
                guard.wakers.push_back(cx.waker().clone());
                return Poll::Pending;
            }
        }
    }
}

//------------------------------------------------------------------------------
//  Mutexの内部状態
//------------------------------------------------------------------------------
struct Inner
{
    wakers: VecDeque<Waker>,
    locked: bool,
}

//------------------------------------------------------------------------------
//  Mutex
//------------------------------------------------------------------------------
pub struct Mutex<T>
{
    inner: std::sync::Mutex<Inner>,
    value: std::sync::Mutex<T>,
}

impl<T> Mutex<T>
{
    //--------------------------------------------------------------------------
    //  Mutexの生成
    //--------------------------------------------------------------------------
    pub fn new( value: T ) -> Mutex<T>
    {
        Self
        {
            inner: std::sync::Mutex::new(Inner
            {
                wakers: VecDeque::new(),
                locked: false,
            }),
            value: std::sync::Mutex::new(value),
        }
    }

    //--------------------------------------------------------------------------
    //  ロックの獲得
    //--------------------------------------------------------------------------
    pub async fn lock( &self ) -> MutexGuard<'_, T>
    {
        LockFuture { mutex: self }.await
    }
}
