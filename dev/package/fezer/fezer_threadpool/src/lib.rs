/*

    スレッドプール

    ----------------------------------------------------------------------------

    # 概要

    単体でもfezerの非同期ランタイムと合わせても利用可能なスレッドプール。

    - パニックになったスレッドは自動的に再起動する
    - スレッドの生成に失敗したときに再試行する
    - `drop()` 時はすべてのアイドルスレッドを停止して自身を削除する
    - `drop()` 時はスレッドがすべて停止するまではメインスレッドをスリープする
    - クロージャか `FnOnce` をスケジュールして、いずれかのスレッドで実行する

    # 使用例

    ```rust
    let pool = fezer_threadpool::ThreadPool::new("worker", 5).unwrap();
    let receiver =
    {
        let (sender, receiver) = std::sync::mpsc::channel();

        for data in data_source
        {
            let sender_clone = sender.clone();
            pool.schedule(move || process_data(data, sender_clone));
        }
        receiver
    };
    let results: Vec<ProcessResult> = receiver.iter().collect();
    ```

    # TODO

    - 性能向上のため、ジョブのスケジューリングにワークスティーリングのキューを採用

*/

#![allow(dead_code)]

mod atomic_counter;
mod error;
mod threadpool;

pub use threadpool::ThreadPool;
