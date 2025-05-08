mod scraper;

use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    println!("Starting scraping...");
    scraper::scrape_sites()?;
    println!("Done.");
    Ok(())
}
