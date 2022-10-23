/*

    非同期チャネル

*/

use core::future::Future;
use core::pin::Pin;
use core::task::{ Context, Poll };
use std::any::type_name;
use std::cell::Cell;
use std::fmt::{ Debug, Formatter };
use std::sync::mpsc::{ RecvError, SendError, TryRecvError, TrySendError };
use std::sync::{ Arc, Mutex };
use std::task::Waker;

//------------------------------------------------------------------------------
//  Inner
//------------------------------------------------------------------------------
pub struct Inner
{
    sender_wakers: Vec<Waker>,
    receiver_waker: Option<Waker>,
}

//------------------------------------------------------------------------------
//  OneSender
//------------------------------------------------------------------------------
pub struct OneSender<T: Send>
{
    std_sender: Option<std::sync::mpsc::SyncSender<T>>,
    inner: Arc<Mutex<Inner>>,
}

impl<T: Send> OneSender<T>
{
    //--------------------------------------------------------------------------
    //  send
    //--------------------------------------------------------------------------
    pub fn send( mut self, value: T ) -> Result<(), SendError<T>>
    {
        self.std_sender.take().unwrap().send(value)
    }
}

impl<T: Send> Drop for OneSender<T>
{
    //--------------------------------------------------------------------------
    //  drop
    //--------------------------------------------------------------------------
    fn drop( &mut self )
    {
        let mut inner_guard = self.inner.lock().unwrap();
        self.std_sender.take();
        let opt_waker = inner_guard.receiver_waker.take();
        drop(inner_guard);
        if let Some(waker) = opt_waker
        {
            waker.wake();
        }
    }
}

impl<T: Send> Debug for OneSender<T>
{
    //--------------------------------------------------------------------------
    //  fmt
    //--------------------------------------------------------------------------
    fn fmt( &self, f: &mut Formatter<'_> ) -> std::fmt::Result
    {
        write!(f, "OneSender<{}>", type_name::<T>())
    }
}

//------------------------------------------------------------------------------
//  SendFut
//------------------------------------------------------------------------------
pub struct SendFut<T: Send>
{
    std_sender: std::sync::mpsc::SyncSender<T>,
    inner: Arc<Mutex<Inner>>,
    value: Cell<Option<T>>,
}

impl<T: Send> Future for SendFut<T>
{
    type Output = Result<(), SendError<T>>;

    //--------------------------------------------------------------------------
    //  poll
    //--------------------------------------------------------------------------
    fn poll( self: Pin<&mut Self>, cx: &mut Context<'_> ) -> Poll<Self::Output>
    {
        let value = self.value.take().take().unwrap();
        let mut inner_guard = self.inner.lock().unwrap();
        match self.std_sender.try_send(value)
        {
            Ok(()) => Poll::Ready(Ok(())),
            Err(TrySendError::Disconnected(value)) => Poll::Ready(Err(SendError(value))),
            Err(TrySendError::Full(value)) =>
            {
                self.value.set(Some(value));
                inner_guard.sender_wakers.push(cx.waker().clone());
                Poll::Pending
            },
        }
    }
}

impl<T: Send> PartialEq for OneSender<T>
{
    //--------------------------------------------------------------------------
    //  eq
    //--------------------------------------------------------------------------
    fn eq( &self, _other: &Self ) -> bool
    {
        false
    }
}

impl<T: Send> Eq for OneSender<T> {}

//------------------------------------------------------------------------------
//  SyncSender
//------------------------------------------------------------------------------
#[derive(Clone)]
pub struct SyncSender<T: Send>
{
    std_sender: Option<std::sync::mpsc::SyncSender<T>>,
    inner: Arc<Mutex<Inner>>,
}

impl<T: Send + Clone> SyncSender<T>
{
    //--------------------------------------------------------------------------
    //  async_send
    //--------------------------------------------------------------------------
    pub async fn async_send( &self, value: T ) -> Result<(), SendError<T>>
    {
        self.wake_receiver_if_ok(
            SendFut
            {
                std_sender: self.std_sender.as_ref().unwrap().clone(),
                inner: self.inner.clone(),
                value: Cell::new(Some(value)),
            }
            .await
        )
    }
}

impl<T: Send> SyncSender<T>
{
    //--------------------------------------------------------------------------
    //  wake_receiver
    //--------------------------------------------------------------------------
    fn wake_receiver( &self )
    {
        let opt_waker = self.inner.lock().unwrap().receiver_waker.take();
        if let Some(waker) = opt_waker
        {
            waker.wake();
        }
    }

    //--------------------------------------------------------------------------
    //  wake_receiver_if_ok
    //--------------------------------------------------------------------------
    fn wake_receiver_if_ok<E>( &self, result: Result<(), E> ) -> Result<(), E>
    {
        if result.is_ok()
        {
            self.wake_receiver();
        }
        result
    }

    //--------------------------------------------------------------------------
    //  send
    //--------------------------------------------------------------------------
    pub fn send( &self, value: T ) -> Result<(), SendError<T>>
    {
        self.wake_receiver_if_ok(self.std_sender.as_ref().unwrap().send(value))
    }

    //--------------------------------------------------------------------------
    //  try_send
    //--------------------------------------------------------------------------
    pub fn try_send( &self, value: T ) -> Result<(), std::sync::mpsc::TrySendError<T>>
    {
        self.wake_receiver_if_ok(self.std_sender.as_ref().unwrap().try_send(value))
    }
}

impl<T: Send> Drop for SyncSender<T>
{
    //--------------------------------------------------------------------------
    //  drop
    //--------------------------------------------------------------------------
    fn drop( &mut self )
    {
        let mut inner_guard = self.inner.lock().unwrap();
        self.std_sender.take();
        if Arc::strong_count(&self.inner) < 3
        {
            let opt_waker = inner_guard.receiver_waker.take();
            drop(inner_guard);
            if let Some(waker) = opt_waker
            {
                waker.wake();
            }
        }
    }
}

impl<T: Send> Debug for SyncSender<T>
{
    //--------------------------------------------------------------------------
    //  fmt
    //--------------------------------------------------------------------------
    fn fmt( &self, f: &mut Formatter<'_> ) -> std::fmt::Result
    {
        write!(f, "SyncSender<{}>", type_name::<T>())
    }
}

impl<T: Send> PartialEq for SyncSender<T>
{
    //--------------------------------------------------------------------------
    //  eq
    //--------------------------------------------------------------------------
    fn eq( &self, other: &Self ) -> bool
    {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl<T: Send> Eq for SyncSender<T> {}

//------------------------------------------------------------------------------
//  Receiver
//------------------------------------------------------------------------------
pub struct Receiver<T>
where
    T: Send,
{
    std_receiver: Option<std::sync::mpsc::Receiver<T>>,
    inner: Arc<Mutex<Inner>>,
}

impl<T: Send> Receiver<T>
{
    //--------------------------------------------------------------------------
    //  wake_senders
    //--------------------------------------------------------------------------
    fn wake_senders( &self )
    {
        let wakers: Vec<Waker> = std::mem::take(&mut self.inner.lock().unwrap().sender_wakers);
        for waker in wakers
        {
            waker.wake();
        }
    }

    //--------------------------------------------------------------------------
    //  wake_senders_if_ok
    //--------------------------------------------------------------------------
    fn wake_senders_if_ok<E>( &self, result: Result<T, E> ) -> Result<T, E>
    {
        if result.is_ok()
        {
            self.wake_senders();
        }
        result
    }

    //--------------------------------------------------------------------------
    //  async_recv
    //--------------------------------------------------------------------------
    pub async fn async_recv( &mut self ) -> Result<T, std::sync::mpsc::RecvError>
    {
        self.await
    }

    //--------------------------------------------------------------------------
    //  try_recv
    //--------------------------------------------------------------------------
    pub fn try_recv( &self ) -> Result<T, std::sync::mpsc::TryRecvError>
    {
        self.wake_senders_if_ok(self.std_receiver.as_ref().unwrap().try_recv())
    }

    //--------------------------------------------------------------------------
    //  recv
    //--------------------------------------------------------------------------
    pub fn recv( &self ) -> Result<T, std::sync::mpsc::RecvError>
    {
        self.wake_senders_if_ok(self.std_receiver.as_ref().unwrap().recv())
    }

    //--------------------------------------------------------------------------
    //  recv_timeout
    //--------------------------------------------------------------------------
    pub fn recv_timeout(
        &self,
        timeout: core::time::Duration
    ) -> Result<T, std::sync::mpsc::RecvTimeoutError>
    {
        self.wake_senders_if_ok(self.std_receiver.as_ref().unwrap().recv_timeout(timeout))
    }

    //--------------------------------------------------------------------------
    //  recv_deadline
    //--------------------------------------------------------------------------
    #[cfg(unstble)]
    pub fn recv_deadline(
        &self,
        deadline: std::time::Instant,
    ) -> Result<T, std::sync::mpsc::RecvTimeoutError>
    {
        self.wake_senders_if_ok(self.std_receiver.as_ref().unwrap().recv_deadline(deadline))
    }

    //--------------------------------------------------------------------------
    //  iter
    //--------------------------------------------------------------------------
    pub fn iter( &self ) -> Iter<'_, T>
    {
        Iter { rx: self }
    }

    //--------------------------------------------------------------------------
    //  try_iter
    //--------------------------------------------------------------------------
    pub fn try_iter( &self ) -> TryIter<'_, T>
    {
        TryIter { rx: self }
    }
}

impl<T: Send> Drop for Receiver<T>
{
    //--------------------------------------------------------------------------
    //  drop
    //--------------------------------------------------------------------------
    fn drop( &mut self )
    {
        let mut inner_guard = self.inner.lock().unwrap();
        self.std_receiver.take();
        let receiver_waker = inner_guard.receiver_waker.take();
        let sender_wakers: Vec<Waker> = std::mem::take(&mut inner_guard.sender_wakers);
        drop(inner_guard);
        drop(receiver_waker);
        for waker in sender_wakers
        {
            waker.wake();
        }
    }
}

impl<T: Send> Future for Receiver<T>
{
    type Output = Result<T, std::sync::mpsc::RecvError>;

    //--------------------------------------------------------------------------
    //  poll
    //--------------------------------------------------------------------------
    fn poll( self: Pin<&mut Self>, cx: &mut Context<'_> ) -> Poll<Self::Output>
    {
        let mut inner_guard = self.inner.lock().unwrap();
        match self.std_receiver.as_ref().unwrap().try_recv()
        {
            Ok(value) =>
            {
                drop(inner_guard);
                self.wake_senders();
                Poll::Ready(Ok(value))
            },
            Err(TryRecvError::Disconnected) => Poll::Ready(Err(RecvError)),
            Err(TryRecvError::Empty) =>
            {
                let waker = cx.waker().clone();
                if Arc::strong_count(&self.inner) < 2
                {
                    Poll::Ready(Err(RecvError))
                }
                else
                {
                    let opt_waker = inner_guard.receiver_waker.replace(waker);
                    drop(inner_guard);
                    drop(opt_waker);
                    Poll::Pending
                }
            },
        }
    }
}

impl<T: Send> Debug for Receiver<T>
{
    //--------------------------------------------------------------------------
    //  fmt
    //--------------------------------------------------------------------------
    fn fmt( &self, f: &mut Formatter<'_> ) -> std::fmt::Result
    {
        write!(f, "Receiver<{}>", type_name::<T>())
    }
}

impl<T: Send> PartialEq for Receiver<T>
{
    //--------------------------------------------------------------------------
    //  eq
    //--------------------------------------------------------------------------
    fn eq( &self, _other: &Self ) -> bool
    {
        false
    }
}

impl<T: Send> Eq for Receiver<T> {}

impl<'a, T: Send> IntoIterator for &'a Receiver<T>
{
    type Item = T;
    type IntoIter = Iter<'a, T>;

    //--------------------------------------------------------------------------
    //  into_iter
    //--------------------------------------------------------------------------
    fn into_iter( self ) -> Iter<'a, T>
    {
        self.iter()
    }
}

//------------------------------------------------------------------------------
//  Iter
//------------------------------------------------------------------------------
#[derive(Debug)]
pub struct Iter<'a, T: 'a + Send>
{
    rx: &'a Receiver<T>,
}

impl<'a, T: Send> Iterator for Iter<'a, T>
{
    type Item = T;

    //--------------------------------------------------------------------------
    //  next
    //--------------------------------------------------------------------------
    fn next( &mut self ) -> Option<T>
    {
        self.rx.recv().ok()
    }
}

//------------------------------------------------------------------------------
//  IntoIter
//------------------------------------------------------------------------------
#[derive(Debug)]
pub struct IntoIter<T: Send>
{
    rx: Receiver<T>,
}

impl<T: Send> Iterator for IntoIter<T>
{
    type Item = T;

    //--------------------------------------------------------------------------
    //  next
    //--------------------------------------------------------------------------
    fn next( &mut self ) -> Option<T>
    {
        self.rx.recv().ok()
    }
}

impl<T: Send> IntoIterator for Receiver<T>
{
    type Item = T;
    type IntoIter = IntoIter<T>;

    //--------------------------------------------------------------------------
    //  into_iter
    //--------------------------------------------------------------------------
    fn into_iter( self ) -> IntoIter<T>
    {
        IntoIter { rx: self }
    }
}

//------------------------------------------------------------------------------
//  TryIter
//------------------------------------------------------------------------------
#[derive(Debug)]
pub struct TryIter<'a, T: 'a + Send>
{
    rx: &'a Receiver<T>,
}

impl<'a, T: Send> Iterator for TryIter<'a, T>
{
    type Item = T;

    //--------------------------------------------------------------------------
    //  next
    //--------------------------------------------------------------------------
    fn next( &mut self ) -> Option<T>
    {
        self.rx.try_recv().ok()
    }
}

//------------------------------------------------------------------------------
//  oneshot
//------------------------------------------------------------------------------
#[must_use]
pub fn oneshot<T>() -> (OneSender<T>, Receiver<T>)
where
    T: Send,
{
    let (std_sender, std_receiver) = std::sync::mpsc::sync_channel(1);
    let inner = Arc::new(Mutex::new(Inner
    {
        sender_wakers: Vec::new(),
        receiver_waker: None,
    }));

    (
        OneSender
        {
            std_sender: Some(std_sender),
            inner: inner.clone(),
        },
        Receiver
        {
            std_receiver: Some(std_receiver),
            inner,
        },
    )
}

//------------------------------------------------------------------------------
//  sync_channel
//------------------------------------------------------------------------------
#[must_use]
pub fn sync_channel<T>( bound: usize ) -> (SyncSender<T>, Receiver<T>)
where
    T: Send,
{
    assert!(bound > 0, "bound must be greater than zero");
    let (std_sender, std_receiver) = std::sync::mpsc::sync_channel(bound);
    let inner = Arc::new(Mutex::new(Inner
    {
        sender_wakers: Vec::new(),
        receiver_waker: None,
    }));

    (
        SyncSender
        {
            std_sender: Some(std_sender),
            inner: inner.clone(),
        },
        Receiver
        {
            std_receiver: Some(std_receiver),
            inner,
        },
    )
}
