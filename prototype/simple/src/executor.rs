use crate::task::Task;

use std::future::Future;
use std::rc::Rc;
use std::sync::mpsc::{ channel, Receiver, Sender };

//------------------------------------------------------------------------------
//  Executor
//------------------------------------------------------------------------------
pub struct Executor
{
    //  実行するタスクのキュー
    task_queue: Receiver<Rc<Task>>,

    //  タスクキューの送信エンドポイント
    task_sender: Sender<Rc<Task>>,
}

impl Executor
{
    //--------------------------------------------------------------------------
    //  Executorを生成
    //--------------------------------------------------------------------------
    pub fn new() -> Executor
    {
        let (task_sender, task_queue) = channel();
        Executor
        {
            task_queue,
            task_sender,
        }
    }

    //--------------------------------------------------------------------------
    //  タスクを生成
    //--------------------------------------------------------------------------
    pub fn spawn( &self, future: impl Future<Output = ()> + 'static )
    {
        let task = Task::new(future, self.task_sender.clone());
        self.task_sender.send(Rc::new(task)).unwrap();
    }

    //--------------------------------------------------------------------------
    //  Executorを起動
    //--------------------------------------------------------------------------
    pub fn run( self )
    {
        //  SenderをdropしてExecutorが追加のタスクを受信しないようにする
        drop(self.task_sender);

        while let Ok(task) = self.task_queue.recv()
        {
            //  タスクを実行
            let _ = task.poll();
        }
    }
}

impl Default for Executor
{
    fn default() -> Executor
    {
        Executor::new()
    }
}
