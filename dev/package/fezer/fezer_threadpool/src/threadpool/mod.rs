/*

    スレッドプール

    ----------------------------------------------------------------------------

    # 概要

    スレッドのコレクションと、実行するジョブのキュー。

    スレッドはパニックになるジョブを実行すると停止し、1つ以上のスレッドが残って
    いれば自動的にスレッドが再起動される。また、新しいジョブをスケジュールしたと
    きにも起動中のスレッド数のチェックと不足分のスレッドの再起動が実行される。

*/

mod inner;

use crate::atomic_counter::AtomicCounter;
use crate::error::{ NewThreadPoolError, StartThreadsError, TryScheduleError };
use crate::threadpool::inner::Inner;

use core::fmt::{ Debug, Formatter };
use core::time::Duration;
use std::sync::mpsc::{ sync_channel, SyncSender, TrySendError };
use std::sync::{ Arc, Mutex };
use std::convert::Into;
use std::time::Instant;

//------------------------------------------------------------------------------
//  スレッドのスリープ
//------------------------------------------------------------------------------
fn sleep_ms( ms: u64 )
{
    std::thread::sleep(Duration::from_millis(ms));
}

//------------------------------------------------------------------------------
//  ThreadPool
//------------------------------------------------------------------------------
pub struct ThreadPool
{
    //  スレッドのコレクション
    inner: Arc<Inner>,

    //  ジョブのSender
    sender: SyncSender<Box<dyn FnOnce() + Send>>,
}

impl ThreadPool
{
    //--------------------------------------------------------------------------
    //  新しいスレッドプールを生成
    //--------------------------------------------------------------------------
    pub fn new( name: &'static str, size: usize ) -> Result<Self, NewThreadPoolError>
    {
        //  名前が指定されていなかった場合
        if name.is_empty()
        {
            return Err
            (
                NewThreadPoolError::Parameter("ThreadPool::new called with empty name".to_string())
            )
        }

        //  スレッド数の指定が0以下だった場合
        if size < 1
        {
            return Err
            (
                NewThreadPoolError::Parameter
                (
                    format!
                    (
                        "ThreadPool::new called with invalid size value: {:?}",
                        size
                    )
                )
            )
        }

        let (sender, receiver) = sync_channel(size * 200);
        let inner = Inner
        {
            name,
            next_name_num: AtomicCounter::new(),
            size,
            receiver: Mutex::new(receiver),
        };
        let pool = Self
        {
            inner: Arc::new(inner),
            sender,
        };

        //  スレッドの起動
        pool.inner.start_threads()?;

        Ok(pool)
    }

    //--------------------------------------------------------------------------
    //  プールのスレッド数を取得
    //--------------------------------------------------------------------------
    pub fn size( &self ) -> usize
    {
        self.inner.size
    }

    //--------------------------------------------------------------------------
    //  現在起動中のスレッド数を取得
    //--------------------------------------------------------------------------
    pub fn num_live_threads( &self ) -> usize
    {
        self.inner.num_live_threads()
    }

    //--------------------------------------------------------------------------
    //  ジョブをスケジュール
    //--------------------------------------------------------------------------
    pub fn schedule<F: FnOnce() + Send + 'static>( &self, f: F )
    {
        let mut opt_box_f: Option<Box<dyn FnOnce() + Send + 'static>> = Some(Box::new(f));

        loop
        {
            //  スレッドの再起動処理
            match self.inner.start_threads()
            {
                Ok(()) | Err(StartThreadsError::Respawn(_)) => {},
                Err(StartThreadsError::NoThreads(_)) =>
                {
                    sleep_ms(10);
                    continue;
                }
            }

            //  キューにジョブを送信
            opt_box_f = match self.sender.try_send(opt_box_f.take().unwrap())
            {
                Ok(()) => return,
                Err(TrySendError::Disconnected(_)) => unreachable!(),
                Err(TrySendError::Full(box_f)) => Some(box_f),
            };

            //  キューがいっぱいだった場合はスリープしてから再試行
            sleep_ms(10);
        }
    }

    //--------------------------------------------------------------------------
    //  ジョブをスケジュール（再試行なし）
    //--------------------------------------------------------------------------
    pub fn try_schedule( &self, f: impl FnOnce() + Send + 'static ) -> Result<(), TryScheduleError>
    {
        //  キューにジョブを送信
        match self.sender.try_send(Box::new(f))
        {
            Ok(_) => {},
            Err(TrySendError::Disconnected(_)) => unreachable!(),
            Err(TrySendError::Full(_)) => return Err(TryScheduleError::QueueFull),
        }
        self.inner.start_threads().map_err(Into::into)
    }

    //--------------------------------------------------------------------------
    //  ThreadPoolをドロップして、スレッドがすべて停止するまで待つ
    //--------------------------------------------------------------------------
    pub fn join( self )
    {
        let inner = self.inner.clone();
        drop(self);
        while inner.num_live_threads() > 0
        {
            sleep_ms(10);
        }
    }

    //--------------------------------------------------------------------------
    //  ThreadPoolをドロップ
    //  タイムアウトを上限としてスレッドの停止を待つ
    //--------------------------------------------------------------------------
    pub fn try_join( self, timeout: Duration ) -> Result<(), String>
    {
        let inner = self.inner.clone();
        drop(self);
        let deadline = Instant::now() + timeout;
        loop
        {
            if inner.num_live_threads() < 1
            {
                return Ok(());
            }
            if deadline < Instant::now()
            {
                return Err("timed out waiting for ThreadPool workers to stop".to_string());
            }
            sleep_ms(10);
        }
    }
}

impl Debug for ThreadPool
{
    //--------------------------------------------------------------------------
    //  fmt
    //--------------------------------------------------------------------------
    fn fmt( &self, f: &mut Formatter<'_> ) -> Result<(), core::fmt::Error>
    {
        write!
        (
            f,
            "ThreadPool{{{:?}, size={:?}}}",
            self.inner.name,
            self.inner.size
        )
    }
}
