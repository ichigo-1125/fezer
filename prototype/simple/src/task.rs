use crate::waker::from_task;

use std::cell::{ RefCell, UnsafeCell };
use std::future::Future;
use std::sync::mpsc::Sender;
use std::rc::Rc;
use std::pin::Pin;
use std::task::{ Context, Poll };

//------------------------------------------------------------------------------
//  Executorによって実行されるタスク
//------------------------------------------------------------------------------
pub struct Task
{
    //  Futureタスク
    future: UnsafeCell<Box<dyn Future<Output = ()>>>,

    //  Executorにタスクを送信するSender
    pub(crate) task_queue: Sender<Rc<Task>>,
}

impl Task
{
    //--------------------------------------------------------------------------
    //  新しいタスクを生成する
    //--------------------------------------------------------------------------
    pub(crate) fn new(
        future: impl Future<Output = ()> + 'static,
        task_queue: Sender<Rc<Task>>,
    ) -> Task
    {
        Task
        {
            future: UnsafeCell::new(Box::new(future)),
            task_queue,
        }
    }

    //--------------------------------------------------------------------------
    //  タスク（ステートマシン）を次の状態まで進める
    //--------------------------------------------------------------------------
    pub(crate) fn poll( self: Rc<Self> ) -> Poll<()>
    {
        let future = unsafe { &mut *self.future.get() }.as_mut();
        let pin = unsafe { Pin::new_unchecked(future) };

        let task_sender = self.task_queue.clone();

        let waker = from_task(self);
        let mut context = Context::from_waker(&waker);

        CURRENT_TASK_SENDER.with(|cell|
        {
            cell.replace(Some(task_sender));
            let res = pin.poll(&mut context);
            cell.replace(None);
            res
        })
    }

    //--------------------------------------------------------------------------
    //  現在のタスクと同じExecutorで実行される新しいタスクを生成する
    //
    //  ※ Executorまたは非同期関数の外部のコンテキストから呼び出されるとpanic
    //--------------------------------------------------------------------------
    pub fn spawn( future: impl Future<Output = ()> + 'static )
    {
        let task_sender = CURRENT_TASK_SENDER.with(|cell|
        {
            cell.borrow()
                .as_ref()
                .expect("Task::spwn() called from outside an executor")
                .clone()
        });

        let task_sender_c = task_sender.clone();
        let task = Task::new(future, task_sender);
        task_sender_c.send(Rc::new(task)).unwrap();
    }
}

thread_local!
{
    pub(crate) static CURRENT_TASK_SENDER: RefCell<Option<Sender<Rc<Task>>>>
        = RefCell::new(None);
}
