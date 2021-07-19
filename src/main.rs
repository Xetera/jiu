mod image;
mod scraper;
use tokio;

use crate::scraper::{scraper::scrape, PinterestBoardFeed, ScrapeRequestInput};

#[tokio::main]
async fn main() {
    better_panic::install();
    let step = ScrapeRequestInput {
        latest_data: vec![
        //     Image {
        //     id: "175147872998493006".to_owned(),
        //     url: "".to_owned(),
        // }
        ],
    };
    match scrape(
        "175147941697542476|/tyrajai2003/dream-catcher/",
        &PinterestBoardFeed {},
        &step,
    )
    .await
    {
        Ok(result) => {
            println!("{:?}", result.images.len());
            // println!("{:?}", result)
        }
        Err(oops) => println!("{:?}", oops),
    };
    println!("hello world");
}

#[test]
fn foo_test() {
    assert_eq!(1, 1);
}
