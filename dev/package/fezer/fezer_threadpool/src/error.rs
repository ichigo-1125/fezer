/*

    スレッドプール用のエラー定義

*/

use core::fmt::{ Debug, Display, Formatter };
use std::error::Error;
use std::io::ErrorKind;

//------------------------------------------------------------------------------
//  エラーの比較
//------------------------------------------------------------------------------
fn err_eq( a: &std::io::Error, b: &std::io::Error ) -> bool
{
    a.kind() == b.kind() && format!("{}", a) == format!("{}", b)
}

//------------------------------------------------------------------------------
//  スレッド立ち上げ時のエラー
//------------------------------------------------------------------------------
#[derive(Debug)]
pub enum StartThreadsError
{
    //  プールにスレッドがない場合
    //  `std::thread::Builder::spawn()` のエラー
    NoThreads(std::io::Error),

    //  プールにスレッドが1つ以上あるが、スレッド起動時にエラーが発生した場合
    //  `std::thread::Builder::spawn()` のエラー
    Respawn(std::io::Error),
}

impl Display for StartThreadsError
{
    //--------------------------------------------------------------------------
    //  fmt
    //--------------------------------------------------------------------------
    fn fmt( &self, f: &mut Formatter<'_> ) -> Result<(), std::fmt::Error>
    {
        match self
        {
            StartThreadsError::NoThreads(e) =>
            {
                write!
                (
                    f,
                    "ThreadPool workers all panicked, failed starting replacement threads: {}",
                    e
                )
            },
            StartThreadsError::Respawn(e) =>
            {
                write!
                (
                    f,
                    "ThreadPool failed starting threads to replace panicked threads: {}",
                    e
                )
            },
        }
    }
}

impl Error for StartThreadsError {}

impl PartialEq for StartThreadsError
{
    //--------------------------------------------------------------------------
    //  eq
    //--------------------------------------------------------------------------
    fn eq( &self, other: &Self ) -> bool
    {
        match (self, other)
        {
            (StartThreadsError::NoThreads(a), StartThreadsError::NoThreads(b))
            | (StartThreadsError::Respawn(a), StartThreadsError::Respawn(b)) => err_eq(a, b),
            _ => false,
        }
    }
}

impl Eq for StartThreadsError {}

//------------------------------------------------------------------------------
//  スレッドプール立ち上げ時のエラー
//------------------------------------------------------------------------------
#[derive(Debug)]
pub enum NewThreadPoolError
{
    //  パラメータが不正だった場合
    Parameter(String),

    //  プール内のスレッド生成時（StartThreadsError）
    //  `std::thread::Builder::spawn()` のエラー
    Spawn(std::io::Error),
}

impl Display for NewThreadPoolError
{
    //--------------------------------------------------------------------------
    //  fmt
    //--------------------------------------------------------------------------
    fn fmt( &self, f: &mut Formatter<'_> ) -> Result<(), std::fmt::Error>
    {
        match self
        {
            NewThreadPoolError::Parameter(s) => write!(f, "{}", s),
            NewThreadPoolError::Spawn(e) =>
            {
                write!(f, "ThreadPool failed starting threads: {}", e)
            },
        }
    }
}

impl Error for NewThreadPoolError {}

impl PartialEq for NewThreadPoolError
{
    //--------------------------------------------------------------------------
    //  eq
    //--------------------------------------------------------------------------
    fn eq( &self, other: &Self ) -> bool
    {
        match (self, other)
        {
            (NewThreadPoolError::Parameter(a), NewThreadPoolError::Parameter(b)) => a == b,
            (NewThreadPoolError::Spawn(a), NewThreadPoolError::Spawn(b)) => err_eq(a, b),
            _ => false,
        }
    }
}

impl From<StartThreadsError> for NewThreadPoolError
{
    //--------------------------------------------------------------------------
    //  from
    //--------------------------------------------------------------------------
    fn from( err: StartThreadsError ) -> Self
    {
        match err
        {
            StartThreadsError::NoThreads(e) | StartThreadsError::Respawn(e) =>
            {
                NewThreadPoolError::Spawn(e)
            }
        }
    }
}

impl From<NewThreadPoolError> for std::io::Error
{
    //--------------------------------------------------------------------------
    //  from
    //--------------------------------------------------------------------------
    fn from( new_thread_pool_error: NewThreadPoolError ) -> Self
    {
        match new_thread_pool_error
        {
            NewThreadPoolError::Parameter(s) =>
            {
                std::io::Error::new(ErrorKind::InvalidInput, s)
            },
            NewThreadPoolError::Spawn(s) =>
            {
                std::io::Error::new(ErrorKind::Other, format!("failed to start threads: {}", s))
            },
        }
    }
}

//------------------------------------------------------------------------------
//  タスクスケジュール時のエラー
//------------------------------------------------------------------------------
#[derive(Debug)]
pub enum TryScheduleError
{
    //  タスクキューが一杯の場合
    QueueFull,

    //  プールにスレッドがない場合（StartThreadsError）
    NoThreads(std::io::Error),

    //  プールにスレッドが1つ以上あるが、スレッド起動時にエラーが発生した場合
    //  （StartThreadsError）
    Respawn(std::io::Error),
}

impl Display for TryScheduleError
{
    //--------------------------------------------------------------------------
    //  fmt
    //--------------------------------------------------------------------------
    fn fmt( &self, f: &mut Formatter<'_> ) -> Result<(), std::fmt::Error>
    {
        match self
        {
            TryScheduleError::QueueFull => write!(f, "ThreadPool queue is full"),
            TryScheduleError::NoThreads(e) =>
            {
                write!
                (
                    f,
                    "ThreadPool workers all panicked, failed starting replacement threads: {}",
                    e
                )
            },
            TryScheduleError::Respawn(e) =>
            {
                write!
                (
                    f,
                    "ThreadPool failed starting threads to replace panicked threads: {}",
                    e
                )
            },
        }
    }
}

impl Error for TryScheduleError {}

impl PartialEq for TryScheduleError
{
    //--------------------------------------------------------------------------
    //  eq
    //--------------------------------------------------------------------------
    fn eq( &self, other: &Self ) -> bool
    {
        match (self, other)
        {
            (TryScheduleError::QueueFull, TryScheduleError::QueueFull) => true,
            (TryScheduleError::NoThreads(a), TryScheduleError::NoThreads(b))
            | (TryScheduleError::Respawn(a), TryScheduleError::Respawn(b)) => err_eq(a, b),
            _ => false,
        }
    }
}

impl Eq for TryScheduleError {}

impl From<StartThreadsError> for TryScheduleError
{
    //--------------------------------------------------------------------------
    //  from
    //--------------------------------------------------------------------------
    fn from( err: StartThreadsError ) -> Self
    {
        match err
        {
            StartThreadsError::NoThreads(e) => TryScheduleError::NoThreads(e),
            StartThreadsError::Respawn(e) => TryScheduleError::Respawn(e),
        }
    }
}

impl From<TryScheduleError> for std::io::Error
{
    //--------------------------------------------------------------------------
    //  from
    //--------------------------------------------------------------------------
    fn from( try_schedule_error: TryScheduleError ) -> Self
    {
        match try_schedule_error
        {
            TryScheduleError::QueueFull =>
            {
                std::io::Error::new(ErrorKind::WouldBlock, "TryScheduleError::QueueFull")
            },
            TryScheduleError::NoThreads(e) =>
            {
                std::io::Error::new
                (
                    e.kind(),
                    format!
                    (
                        "ThreadPool workers all panicked, failed starting replacement threads: {}",
                        e
                    )
                )
            },
            TryScheduleError::Respawn(e) =>
            {
                std::io::Error::new
                (
                    e.kind(),
                    format!
                    (
                        "ThreadPool failed starting threads to replace panicked threads: {}",
                        e
                    )
                )
            },
        }
    }
}
