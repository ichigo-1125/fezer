use std::time::Duration;
use fezer::{ async_main, futures::sleep };

async_main!
{
    println!("start");
    foo().await;
    println!("done");
}

async fn foo()
{
    println!("foo");
    let mut result = delay_add(1.0, 3.0).await;
    println!("result: {}", result);
    result = delay_add(result, 5.0).await;
    println!("result: {}", result);
}

async fn delay_add( a: f32, b: f32 ) -> f32
{
    sleep(Duration::from_millis(1000)).await;
    a + b
}
