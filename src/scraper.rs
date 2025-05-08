// scraper.rs
// contains logic for scraping various car rental sites

use chrono::Local;
use config::Config;
use csv::Writer;
use regex::Regex;
use reqwest::blocking::Client;
use scraper::{Html, Selector};
use serde::Deserialize;
use std::error::Error;
use std::fs::File;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

#[derive(Deserialize)]
struct SearchConfig {
    zip: String,
    distance: u32,
}

pub fn scrape_sites() -> Result<(), Box<dyn Error>> {
    // first specify the settings in the config file
    let settings = Config::builder()
        .add_source(config::File::with_name("config.toml"))
        .build()?;

    let search = settings.get::<SearchConfig>("search")?;

    let date_str = Local::now().format("%m%d%Y").to_string();
    let filename = format!("rentals{}.csv", date_str);
    let path = PathBuf::from(&filename);
    let mut wtr = Writer::from_path(&path)?;
    wtr.write_record(&[
        "Site", "VIN", "Year", "Make", "Model", "Trim", "Price", "Mileage", "KBB", "URL",
    ])?;

    scrape_enterprise(&mut wtr, &search.zip, search.distance)?;
    // fill in the rest later

    wtr.flush()?;
    println!("Data saved to {}", filename);
    Ok(())
}

fn scrape_enterprise(
    wtr: &mut Writer<File>,
    zip: &str,
    distance: u32,
) -> Result<(), Box<dyn Error>> {
    // get our reqwest client
    let client = Client::builder().build()?;

    // create our vin Selector
    let vin_selector = Selector::parse("div[data-auto5-vehicle-vin]").unwrap();

    // and a selector for the correct mileage label
    let mileage_selector = Selector::parse("span.label, span.value").unwrap();

    // and a kbb value selector
    let kbb_selector = Selector::parse(".kbbsuggested-row .row-value").unwrap();

    // and define a page count
    let mut page = 1;

    // loop through each page
    loop {
        // create the url with the stuff we need
        let url = format!(
            "https://www.enterprisecarsales.com/list/buy-a-car/distance---{}/srp-page-{}/?zipcode={}",
            distance, page, zip
        );

        // send the request to the page
        let res = client.get(&url).send()?.text()?;

        // convert the response to html type
        let document = Html::parse_document(&res);

        // select the stuff we need
        let mut found = false;

        for node in document.select(&vin_selector) {
            // so get the vins
            if let Some(vin) = node.value().attr("data-auto5-vehicle-vin") {
                found = true;

                // wait a second before calling the api
                thread::sleep(Duration::from_secs(1));

                // construct the url for the api with the vin
                let vehicle_url = format!(
                    "https://www.enterprisecarsales.com/wp-json/jazel-auto5/v1/vehicle/{}",
                    vin
                );

                // get the api response
                let vehicle_res = client
                    .get(&vehicle_url)
                    .send()?
                    .json::<serde_json::Value>()?;

                // now get everything from the api response
                let year = vehicle_res["year"].as_str().unwrap_or("");
                let make = vehicle_res["make"].as_str().unwrap_or("");
                let model = vehicle_res["model"].as_str().unwrap_or("");
                let trim = vehicle_res["trim"].as_str().unwrap_or("");
                let price = vehicle_res["displayPrice"].to_string();
                let vdp_url = vehicle_res["vdp_url"].as_str().unwrap_or("");

                // wait again before vdp
                thread::sleep(Duration::from_secs(1));

                // and we need to collect the mileage from the vdp url
                let vdp_res = client.get(vdp_url).send()?.text()?;
                // parse to html
                let vdp_doc = Html::parse_document(&vdp_res);

                let mut mileage = "Unknown".to_string(); // write our record
                let mut kbb_price = "Unknown".to_string();

                let mut last_was_label = false;
                for element in vdp_doc.select(&mileage_selector) {
                    let text = element.text().collect::<String>().trim().to_string();
                    if last_was_label {
                        mileage = text;
                        break;
                    }
                    if text == "Mileage:" {
                        last_was_label = true;
                    }
                }

                if let Some(kbb_elem) = vdp_doc.select(&kbb_selector).next() {
                    kbb_price = kbb_elem.text().collect::<String>().trim().to_string();

                    // Strip $ and commas for CSV
                    let re = Regex::new(r"[^\d.]").unwrap(); // removes everything except digits and decimal points
                    kbb_price = re.replace_all(&kbb_price, "").to_string();
                }

                wtr.write_record(&[
                    "Enterprise",
                    vin,
                    year,
                    make,
                    model,
                    trim,
                    &price,
                    &mileage,
                    &kbb_price,
                    vdp_url,
                ])?;
                println!(
                    "Added: {} {} {} {} - ${} - {} miles",
                    year, make, model, trim, price, mileage
                );
                // println!("{:#?}", vehicle_res);
            }
        }

        if !found {
            break;
        }

        page += 1;
    }

    Ok(())
}
