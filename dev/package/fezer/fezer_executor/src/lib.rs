//  use core::cell::Cell;
//  use core::future::Future;
//  use core::pin::Pin;
//  use core::task::Poll;
//  use std::sync::mpsc::SyncSender;
//  use std::sync::{ Arc, Mutex, Weak };

thread_local!
{
//    static EXECUTOR: Cell<Weak<Executor>> = Cell::new(Weak::new());
}
