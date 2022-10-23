/*

    スレッドのコレクション

*/

use crate::error::StartThreadsError;
use crate::atomic_counter::AtomicCounter;

use core::sync::atomic::AtomicUsize;
use core::time::Duration;
use std::sync::{ Arc, Mutex };
use std::sync::mpsc::{ Receiver, RecvTimeoutError };

//  スレッド数の上限
pub static INTERNAL_MAX_THREADS: AtomicUsize = AtomicUsize::new(usize::MAX);

//------------------------------------------------------------------------------
//  Inner
//------------------------------------------------------------------------------
pub(crate) struct Inner
{
    //  スレッドに関連付けられた名前
    pub(crate) name: &'static str,

    //  nameのサフィックスとして付与されるカウント
    pub(crate) next_name_num: AtomicCounter,

    //  スレッド数
    pub(crate) size: usize,

    //  ジョブのReceiver
    //  複数のスレッドからアクセスされる可能性があるので排他制御が必要
    pub(crate) receiver: Mutex<Receiver<Box<dyn FnOnce() + Send>>>,
}

impl Inner
{
    //--------------------------------------------------------------------------
    //  生存中のスレッドの数
    //--------------------------------------------------------------------------
    pub(crate) fn num_live_threads( self: &Arc<Self> ) -> usize
    {
        Arc::strong_count(self) - 1
    }

    //--------------------------------------------------------------------------
    //  スレッド生成時に実行される処理
    //--------------------------------------------------------------------------
    fn work( self: &Arc<Self> )
    {
        loop
        {
            //  ジョブを受信
            let recv_result = self
                .receiver
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_millis(500));

            match recv_result
            {
                Ok(f) =>
                {
                    //  スレッドが停止していた場合のための再起動処理
                    let _ignored = self.start_threads();

                    //  ジョブを実行
                    f();
                },

                //  タイムアウトの場合は何もしない
                Err(RecvTimeoutError::Timeout) => {},

                //  チャネルに接続できなかった場合はスレッドを停止
                Err(RecvTimeoutError::Disconnected) => return,
            }

            //  スレッドが停止していた場合のための再起動処理
            let _ignored = self.start_threads();
        }
    }

    //--------------------------------------------------------------------------
    //  単一のスレッドを生成
    //--------------------------------------------------------------------------
    fn spawn_thread(
        &self,
        num_live_threads: usize,
        name: String,
        f: impl FnOnce() + Send + 'static,
    ) -> Result<(), std::io::Error>
    {
        //  起動中のスレッド数が上限に達した場合はエラー
        if num_live_threads >= INTERNAL_MAX_THREADS.load(std::sync::atomic::Ordering::Acquire)
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Number of threads reached the upper limit".to_string()
            ));
        }

        //  スレッドを生成
        std::thread::Builder::new().name(name).spawn(f)?;
        Ok(())
    }

    //--------------------------------------------------------------------------
    //  単一のスレッドを起動
    //--------------------------------------------------------------------------
    fn start_thread( self: &Arc<Self> ) -> Result<(), StartThreadsError>
    {
        let self_clone = self.clone();
        let num_live_threads = self.num_live_threads() - 1;

        //  起動中のスレッド数が目的のスレッド数よりも小さければ、スレッドを生成
        if num_live_threads < self.size
        {
            if let Err(e) = self.spawn_thread(
                num_live_threads,
                format!("{}-{}", self.name, self.next_name_num.next()),
                move || self_clone.work()
            )
            {
                //  スレッドの起動に失敗した場合はエラー

                if num_live_threads == 0
                {
                    return Err(StartThreadsError::NoThreads(e));
                }
                else
                {
                    return Err(StartThreadsError::Respawn(e));
                }
            };
        }

        Ok(())
    }

    //--------------------------------------------------------------------------
    //  スレッドの一括起動
    //--------------------------------------------------------------------------
    pub(crate) fn start_threads( self: &Arc<Self> ) -> Result<(), StartThreadsError>
    {
        //  プールサイズに達するまでスレッドを起動
        while self.num_live_threads() < self.size
        {
            self.start_thread()?;
        }

        Ok(())
    }
}
