/*

    アトミックなカウンタ

    ----------------------------------------------------------------------------

    # 概要

    複数のスレッドから直接カウントアップしても安全なカウンタ。

    # 使用例

    ```rust
    //  マルチスレッドで共有するカウンタを生成
    let counter = std::sync::Arc::new(AtomicCounter::new());

    assert_eq!(0, counter.next());
    assert_eq!(1, counter.next());
    assert_eq!(2, counter.next());
    ```

    # アトミック変数について

    アトミック変数はマルチスレッディングにおいてデータ競合を起こさないことを保
    証する変数。排他制御よりもコストが低い。

*/

use std::sync::atomic::{ AtomicUsize, Ordering };

//------------------------------------------------------------------------------
//  AtomicCounter
//------------------------------------------------------------------------------
pub(crate) struct AtomicCounter
{
    next_value: AtomicUsize,
}

impl AtomicCounter
{
    //--------------------------------------------------------------------------
    //  新しいアトミックカウンタを生成
    //--------------------------------------------------------------------------
    pub fn new() -> Self
    {
        Self
        {
            next_value: AtomicUsize::new(0),
        }
    }

    //--------------------------------------------------------------------------
    //  カウンタをカウントアップし、元の値を返す
    //--------------------------------------------------------------------------
    pub fn next( &self ) -> usize
    {
        self.next_value.fetch_add(1, Ordering::AcqRel)
    }
}

//------------------------------------------------------------------------------
//  テスト
//------------------------------------------------------------------------------
#[cfg(test)]
mod tests
{
    use super::AtomicCounter;

    //--------------------------------------------------------------------------
    //  test_atomic_counter
    //--------------------------------------------------------------------------
    #[test]
    fn test_atomic_counter()
    {
        let counter = std::sync::Arc::new(AtomicCounter::new());
        assert_eq!(0, counter.next());
        assert_eq!(1, counter.next());
        assert_eq!(2, counter.next());
    }

    //--------------------------------------------------------------------------
    //  test_atomic_counter_many_readers
    //--------------------------------------------------------------------------
    #[test]
    fn test_atomic_counter_many_readers()
    {
        let receiver =
        {
            let counter = std::sync::Arc::new(AtomicCounter::new());
            let (sender, receiver) = std::sync::mpsc::channel();

            //  10個のスレッドからそれぞれ10回ずつカウントアップ
            for _ in 0..10
            {
                let counter_clone = counter.clone();
                let sender_clone = sender.clone();

                std::thread::spawn(move ||
                {
                    for _ in 0..10
                    {
                        //  カウンタを更新
                        sender_clone.send(counter_clone.next()).unwrap();
                    }
                });
            }

            receiver
        };

        let mut values: Vec<usize> = receiver.iter().collect();
        values.sort();
        assert_eq!((0_usize..100).collect::<Vec<usize>>(), values)
    }
}
