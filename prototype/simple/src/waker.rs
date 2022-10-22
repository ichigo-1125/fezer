use crate::task::Task;

use std::rc::Rc;
use std::task::{ RawWaker, RawWakerVTable, Waker };

pub(crate) fn from_task( task: Rc<Task> ) -> Waker
{
    let raw = Rc::into_raw(task);
    let raw_waker = RawWaker::new(raw.cast(), &WAKER_VTABLE);
    unsafe { Waker::from_raw(raw_waker) }
}

const WAKER_VTABLE: RawWakerVTable =
    RawWakerVTable::new(waker_clone, waker_wake, waker_wake_by_ref, waker_drop);

fn waker_clone( ptr: *const () ) -> RawWaker
{
    let rc = unsafe { Rc::<Task>::from_raw(ptr.cast()) };
    let clone = Rc::clone(&rc);
    std::mem::forget(rc);
    RawWaker::new(Rc::into_raw(clone).cast(), &WAKER_VTABLE)
}

fn waker_drop( ptr: *const () )
{
    unsafe
    {
        Rc::<Task>::from_raw(ptr.cast());
    }
}

fn waker_wake( ptr: *const () )
{
    let rc = unsafe { Rc::<Task>::from_raw(ptr.cast()) };
    let task_queue = rc.task_queue.clone();
    task_queue.send(rc).unwrap();
}

fn waker_wake_by_ref( ptr: *const () )
{
    let rc = unsafe { Rc::<Task>::from_raw(ptr.cast()) };
    let rc_c = Rc::clone(&rc);
    rc.task_queue.send(rc_c).unwrap();
    std::mem::forget(rc);
}
