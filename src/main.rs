
use pretty_env_logger;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    let db = models::blank_db();

    _ = handlers::update(db.clone()).await;

    let api = filters::routes(db);

    warp::serve(api).run(([0, 0, 0, 0], 80)).await;
}

mod filters {
    use super::handlers;
    use super::models::Db;
    use warp::Filter;

    pub fn routes(
        db: Db,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        post_update(db.clone())
        .or(get_data(db.clone()))
    }

    pub fn post_update(
        db: Db,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path!("update")
            .and(warp::post())
            .and(is_form_content())
            .and(with_db(db))
            .and_then(handlers::update)
    }

    pub fn get_data(
        db: Db,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
            warp::path!("get")
            .and(warp::get())
            .and(with_db(db))
            .and_then(handlers::get)
    }

    fn with_db(db: Db) -> impl Filter<Extract = (Db,), Error = std::convert::Infallible> + Clone {
        warp::any().map(move || db.clone())
    }

    fn is_form_content() -> impl Filter<Extract = (), Error = warp::Rejection> + Copy {
        warp::header::exact_ignore_case("Content-Type", "application/x-www-form-urlencoded")
    }
}

mod handlers {
    use std::env;

    use log::info;
    use serde_json::{Value};

    use super::models::Db;

    pub async fn update(db: Db) -> Result<impl warp::Reply, warp::Rejection> {
        let data_source_url =
            "https://spinitron.com/api/spins/?access-token=";
        let access_token = match env::var("SPIN_KEY") {
            Ok(v) => v,
            Err(e) => panic!("Couldn't read SPIN_KEY: {}", e),
        };

        let count_url = "&count=5";
        let data_source_url = data_source_url.to_owned() + &access_token + count_url;
        info!("Sending a request to {}", data_source_url);
        let resp = reqwest::get(data_source_url).await.unwrap();

        let str = resp.text().await.unwrap();

        let v: Value = serde_json::from_str(&str).unwrap();

        // Store in db
        let mut db = db.lock().await;
        *db = v.clone();

        Ok(warp::reply::with_status(
            "Fetching update.",
            warp::http::StatusCode::ACCEPTED,
        ))
    }

    pub async fn get(db: Db) -> Result<impl warp::Reply, warp::Rejection> {
        let db = db.lock().await;
        let resp = db.clone();
        Ok(warp::reply::json(&resp["items"]))
    }

    // pub async fn render_response(db: Db) -> Result<impl warp::Reply, warp::Rejection> {
    //     let resp = get_spins(db).await;
    //     match resp {
    //         Ok(_) => Ok(warp::reply::with_status(
    //             "Fetching update.",
    //             warp::http::StatusCode::PROCESSING,
    //         )),
    //         Err(_) => Ok(warp::reply::with_status(
    //             "Failed.",
    //             warp::http::StatusCode::FORBIDDEN,
    //         )),
    //     }
    // }
}

// async fn hello(
//     param: Option<HashMap<String, String>>,
// ) -> Result<impl warp::Reply, warp::Rejection> {
//     // thread::spawn(|| {
//     //     get_spins();
//     // }).join().expect("Thread panicked");

//     // Unpack hashmap
//     match param {
//         Some(param) => {
//             let song_title = param.get("title").unwrap();
//             let artist = param.get("artist").unwrap();
//             println!("song_title: {}, artist: {}", song_title, artist);
//             Ok(warp::reply::with_status(
//                 "Successfully updated current song.",
//                 warp::http::StatusCode::CREATED,
//             ))
//         }
//         None => {
//             println!("No params");
//             Ok(warp::reply::with_status(
//                 "No params",
//                 warp::http::StatusCode::CREATED,
//             ))
//         }
//     }
// }

// fn parse_body(mut body: impl Buf) -> Option<HashMap<String, String>> {
//     if body.remaining() < MIN_LEN {
//         return None;
//     }

//     // Declare hashmap
//     let mut map: HashMap<String, String> = HashMap::new();

//     let mut peek_buffer = vec![0; std::cmp::min(body.remaining(), MAX_LEN)];
//     body.copy_to_slice(&mut peek_buffer);

//     let parts: Vec<&str> = std::str::from_utf8(&peek_buffer)
//         .ok()?
//         .split(|c| c == '=' || c == '&')
//         .collect();

//     println!("parts: {:?}", parts);

//     // Iterate over parts and add to hashmap
//     for i in 0..parts.len() {
//         if i + 1 >= parts.len() {
//             break;
//         }
//         if i % 2 == 0 {
//             map.insert(parts[i].to_string(), parts[i + 1].to_string());
//         }
//     }

//     println!("map: {:?}", map);
//     // Return hashmap
//     return Some(map.clone());
// }

mod models {
    use serde_json::{json, Value};
    use std::sync::Arc;
    use tokio::sync::Mutex;

    pub type Db = Arc<Mutex<Value>>;

    pub fn blank_db() -> Db {
        Arc::new(Mutex::new(json!(null)))
    }
}
