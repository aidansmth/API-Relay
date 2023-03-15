#[macro_use]
extern crate log;

use std::{
    collections::HashMap,
    env,
    sync::{Arc, Mutex},
};
use tokio_cron_scheduler::{Job, JobScheduler};

use handlers::Message;
use tokio::sync::mpsc::UnboundedSender;
use warp::Filter;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    let spin_db = models::blank_db();
    let show_db = models::blank_db();

    // Create cron job to update shows every hour
    let _ = create_cron(show_db.clone()).await;

    let connected_users: Arc<Mutex<HashMap<usize, UnboundedSender<Message>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    _ = handlers::update_spins_no_reply(spin_db.clone()).await;
    _ = handlers::update_shows(show_db.clone()).await;

    let for_closure = connected_users.clone();
    let connected_users_filter = warp::any().map(move || for_closure.clone());

    let spin_recv = warp::path!("spins" / "stream")
        .and(warp::get())
        .and(connected_users_filter)
        .map(|connected_users_filter| {
            let stream = handlers::user_connected(connected_users_filter);
            warp::sse::reply(warp::sse::keep_alive().stream(stream))
        })
        .with(warp::reply::with::headers(headers::cors()));

    let api = spin_recv.or(filters::routes(spin_db, show_db, connected_users.clone()));

    // If env var LOCAL is set, run on localhost
    if env::var("LOCAL").is_ok() {
        info!("Running on localhost");
        warp::serve(api).run(([127, 0, 0, 1], 8080)).await;
    } else {
        info!("Running exposed");
        warp::serve(api).run(([0, 0, 0, 0], 80)).await;
    }
}

async fn create_cron(show_db: models::Db) {
    let scheduler = JobScheduler::new().await;

    let show_db_clone = show_db.clone();

    match scheduler {
        Ok(sched) => {
            // create job that refreshes show every 15 minutes
            let job = Job::new_async("1 0,15,30,45 * * * *", move |_, _| {
                let short_lived_db = show_db_clone.clone();
                Box::pin(async {
                    info!("{:?}: updating shows.", chrono::Utc::now());
                    let _ = handlers::update_shows(short_lived_db).await;
                })
            });
            // add job to scheduler
            match job {
                Ok(job) => match sched.add(job).await {
                    Ok(_) => {
                        info!("Job added to scheduler");
                    }
                    Err(e) => {
                        error!("Error: {}", e);
                    }
                },
                Err(e) => {
                    error!("Error: {}", e);
                }
            }
            // start scheduler
            let _ = sched.start().await;
        }
        Err(e) => {
            error!("Error: {}", e);
        }
    }
}

mod filters {
    use std::convert::Infallible;
    use std::sync::Arc;

    use crate::headers;

    use super::handlers;
    use super::models::Db;
    use serde_json::Value;
    use tokio::sync::Mutex;
    use warp::Filter;

    pub fn routes(
        spin_db: Db,
        show_db: Db,
        users: handlers::Users,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        spin_update(spin_db.clone(), users.clone())
            .or(get_spin(spin_db.clone()))
            .or(show_update(show_db.clone()))
            .or(get_show(show_db.clone()))
            .or(health_check())
            .or(not_found())
    }

    use warp::Reply;

    // Update methods
    pub fn spin_update(
        spin_db: Db,
        users: handlers::Users,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone{
        warp::path!("spins" / "update")
            .and(warp::post())
            .and(is_form_content())
            // .and(with_db(spin_db))
            .and(with_db_and_users(spin_db, users))
            .and_then(
                |(db, users): (Arc<Mutex<Value>>, handlers::Users)| async move {
                    let resp = handlers::update_spins_no_reply(db.clone()).await;
                    match resp {
                        Ok(_) => {
                            trace!("Spins updated");
                        }
                        Err(e) => {
                            error!("Error fetching spins.");
                            return Ok::<_, Infallible>(
                                warp::reply::with_status("Error", warp::http::StatusCode::INTERNAL_SERVER_ERROR)
                                    .into_response(),
                            );
                        }
                    }
                    let _ = handlers::send_update(users.clone());
                    // Must satisfy return type
                    Ok::<_, Infallible>(
                        warp::reply::with_status("OK", warp::http::StatusCode::OK).into_response(),
                    )
                },
            )
    }

    pub fn show_update(
        show_db: Db,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path!("shows" / "update")
            .and(warp::post())
            .and(is_form_content())
            .and(with_db(show_db))
            .and_then(handlers::update_shows)
    }

    // Get methods
    pub fn get_spin(
        spin_db: Db,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path!("spins" / "get")
            .and(warp::get())
            .and(with_db(spin_db))
            .and_then(handlers::get)
            .with(warp::reply::with::headers(headers::cors()))
    }

    pub fn get_show(
        show_db: Db,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path!("shows" / "get")
            .and(warp::get())
            .and(with_db(show_db))
            .and_then(handlers::get)
            .with(warp::reply::with::headers(headers::cors()))
    }

    pub fn health_check() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone
    {
        warp::path!("healthCheck")
            .and(warp::get())
            .map(|| warp::reply::with_status("OK", warp::http::StatusCode::OK))
    }

    pub fn not_found() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::any()
            .and(warp::path::end())
            .map(|| warp::reply::with_status("Not Found", warp::http::StatusCode::NOT_FOUND))
    }

    fn with_db(db: Db) -> impl Filter<Extract = (Db,), Error = std::convert::Infallible> + Clone {
        warp::any().map(move || db.clone())
    }

    fn with_db_and_users(
        db: Db,
        users: handlers::Users,
    ) -> impl Filter<Extract = ((Db, handlers::Users),), Error = std::convert::Infallible> + Clone
    {
        warp::any().map(move || (db.clone(), users.clone()))
    }

    fn is_form_content() -> impl Filter<Extract = (), Error = warp::Rejection> + Copy {
        warp::header::exact_ignore_case("Content-Type", "application/x-www-form-urlencoded")
    }
}
mod handlers {
    use std::{
        collections::HashMap,
        env,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc, Mutex,
        },
    };

    use futures_util::Stream;
    use log::{debug, info};
    use serde_json::{Value, Map};
    use tokio::sync::mpsc;
    use tokio_stream::wrappers::UnboundedReceiverStream;
    use warp::{sse::Event, Reply};

    use futures_util::stream::StreamExt;

    use super::models::Db;

    async fn remove_links_spins(v: Value) -> Value {
        let mut new_v = Value::Object(serde_json::Map::new());
        let arr = v["items"].as_array().unwrap();
        for i in 0..arr.len() {
            let iter = arr[i].as_object().unwrap();
            let mut new_iter = Value::Object(serde_json::Map::new());
            for (key, value) in iter {
                if key != "_links" {
                    new_iter[key] = value.clone();
                }
            }
            new_v["spin-".to_string() + &i.to_string()] = new_iter;
        }
        debug!("new_v: {:?}", new_v);
        new_v
    }

    async fn remove_links_shows(v: Value) -> Value {
        let mut new_v = Value::Object(serde_json::Map::new());
        let arr = v["items"].as_array().unwrap();
        for i in 0..arr.len() {
            let iter = arr[i].as_object().unwrap();
            let mut new_iter = Value::Object(serde_json::Map::new());
            for (key, value) in iter {
                if key != "_links" {
                    new_iter[key] = value.clone();
                }
            }
            new_v["show-".to_string() + &i.to_string()] = new_iter;
        }
        debug!("new_v: {:?}", new_v);
        new_v
    }

    async fn remove_links_djs(v: Value) -> Value {
        let mut new_v = Value::Object(serde_json::Map::new());
        let arr = v.as_object().unwrap();
        for (key, value) in arr {
            if key != "_links" {
                new_v[key] = value.clone();
            }
        }
        debug!("new_v: {:?}", new_v);
        new_v
    }

    pub async fn update_spins_no_reply(db: Db) -> Result<impl warp::Reply, warp::Rejection> {
        info!("POST recieved from Spinitron, updating spins");
        let data_source_url = "https://spinitron.com/api/spins/?access-token=";
        let access_token = match env::var("SPIN_KEY") {
            Ok(v) => v,
            Err(e) => panic!("Couldn't read SPIN_KEY: {}", e),
        };

        let count_url = "&count=5";
        let data_source_url = data_source_url.to_owned() + &access_token + count_url;

        trace!("Sending a request to {}", data_source_url);

        let resp = reqwest::get(data_source_url).await;
        let resp_str;
        
        match resp {
            Err(e) => {
                error!("Couldn't update spins: {}", e);
                return Ok(warp::reply::with_status("Couldn't fetch spins.", warp::http::StatusCode::INTERNAL_SERVER_ERROR));
            }
            Ok(v) => resp_str = v,
        }


        let str = resp_str.text().await.unwrap();

        // If str is empty, return
        if str == "" {
            return Ok(warp::reply::with_status("Response was empty.", warp::http::StatusCode::INTERNAL_SERVER_ERROR));
        }

        let v: Value = serde_json::from_str(&str).unwrap();

        // Store in db
        let mut db = db.lock().await;
        *db = remove_links_spins(v).await;
        return Ok(warp::reply::with_status("Finished updating spins.", warp::http::StatusCode::OK));
    }

    pub async fn update_shows(db: Db) -> Result<impl warp::Reply, warp::Rejection> {
        let data_source_url = "https://spinitron.com/api/shows/?access-token=";
        let access_token = match env::var("SPIN_KEY") {
            Ok(v) => v,
            Err(e) => panic!("Couldn't read SPIN_KEY: {}", e),
        };

        let count_url = "&count=2";
        let data_source_url = data_source_url.to_owned() + &access_token + count_url;
        trace!("Sending a request to {}", data_source_url);
        let resp = reqwest::get(data_source_url).await;
        let resp_str;
        
        match resp {
            Err(e) => {
                error!("Couldn't get shows: {}", e);
                return Ok(warp::reply::with_status("Couldn't fetch shows.", warp::http::StatusCode::INTERNAL_SERVER_ERROR));
            }
            Ok(v) => resp_str = v,
        }

        let str = resp_str.text().await.unwrap();

        // If str is empty, return
        if str == "" {
            return Ok(warp::reply::with_status("Response was empty.", warp::http::StatusCode::INTERNAL_SERVER_ERROR));
        }

        let v: Value = serde_json::from_str(&str).unwrap();

        let dj1 = v["items"][0]["_links"]["personas"][0]["href"]
            .as_str()
            .unwrap();
        let dj2 = v["items"][1]["_links"]["personas"][0]["href"]
            .as_str()
            .unwrap();

        // Fetch DJ info using reqwest
        let dj1_data = reqwest::get(dj1).await.unwrap().text().await;
        let dj2_data = reqwest::get(dj2).await.unwrap().text().await;

        // match both djs, unwraping or returning error
        let dj1_data = match dj1_data {
            Ok(v) => v,
            Err(e) => {
                error!("Couldn't get dj1_data: {}", e);
                return Err(warp::reject::not_found());
            }
        };

        let dj2_data = match dj2_data {
            Ok(v) => v,
            Err(e) => {
                error!("Couldn't get dj2_data: {}", e);
                return Err(warp::reject::not_found());
            }
        };

        let dj1_data: Value = remove_links_djs(serde_json::from_str(&dj1_data).unwrap()).await;
        let dj2_data: Value = remove_links_djs(serde_json::from_str(&dj2_data).unwrap()).await;

        // Remove links
        let mut new_v = remove_links_shows(v).await;

        new_v["dj-0"] = dj1_data;
        new_v["dj-1"] = dj2_data;

        // Store in db
        let mut db = db.lock().await;
        // Remove _links field
        *db = new_v;

        Ok(warp::reply::with_status(
            "Finished updating shows and DJs.",
            warp::http::StatusCode::OK,
        ))
    }

    pub async fn get(db: Db) -> Result<impl warp::Reply, warp::Rejection> {
        let db = db.lock().await;
        let resp = db.clone();
        if resp == Value::Null {
            // Create json object with 500 error and return
            let mut resp = Map::new();
            resp.insert("error".to_string(), Value::String("500".to_string()));
            return Ok(warp::reply::with_status(
                warp::reply::json(&resp),
                warp::http::StatusCode::INTERNAL_SERVER_ERROR,
            ));
        }
        Ok(warp::reply::with_status(
            warp::reply::json(&resp),
            warp::http::StatusCode::OK,
        ))
    }

    pub(crate) type Users = Arc<Mutex<HashMap<usize, mpsc::UnboundedSender<Message>>>>;
    static NEXT_USER_ID: std::sync::atomic::AtomicUsize = AtomicUsize::new(1);

    #[derive(Debug)]
    pub enum Message {
        UserId(usize),
        Reply(String),
    }

    #[derive(Debug)]
    struct NotUtf8;
    impl warp::reject::Reject for NotUtf8 {}

    pub(crate) fn user_connected(
        users: Users,
    ) -> impl Stream<Item = Result<Event, warp::Error>> + Send + 'static {
        let my_id = NEXT_USER_ID.fetch_add(1, Ordering::Relaxed);

        // Use an unbounded channel to handle buffering and flushing of messages
        // to the event source...
        let (tx, rx) = mpsc::unbounded_channel();
        let rx = UnboundedReceiverStream::new(rx);

        tx.send(Message::UserId(my_id))
            // rx is right above, so this cannot fail
            .unwrap();

        // Save the sender in our list of connected users.
        users.lock().unwrap().insert(my_id, tx);

        // Convert messages into Server-Sent Events and return resulting stream.
        rx.map(|msg| match msg {
            Message::UserId(_my_id) => Ok(Event::default()
                .event("user")
                .data("Connected.".to_string())),
            Message::Reply(reply) => Ok(Event::default().data(reply)),
        })
    }

    pub(crate) fn send_update(users: Users) {
        users.lock().unwrap().retain(|_uid, tx| {
            tx.send(Message::Reply("Spin outdated - Update needed.".to_string()))
                .is_ok()
        });
    }
}

mod headers {
    use warp::http::header::{HeaderMap, HeaderValue};

    // create headers to be used in responses
    pub fn cors() -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            "Access-Control-Allow-Origin",
            HeaderValue::from_static("*"),
        );
        headers.insert(
            "Access-Control-Allow-Methods",
            HeaderValue::from_static("GET, POST, PUT, DELETE, OPTIONS"),
        );
        headers.insert(
            "Access-Control-Allow-Headers",
            HeaderValue::from_static("Content-Type, Authorization"),
        );
        headers
    }
}

mod models {
    use serde_json::{json, Value};
    use std::sync::Arc;
    use tokio::sync::Mutex;

    pub type Db = Arc<Mutex<Value>>;

    pub fn blank_db() -> Db {
        Arc::new(Mutex::new(json!(null)))
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    };

    use tokio::sync::mpsc::UnboundedSender;
    use warp::http::StatusCode;
    use warp::test::request;

    use crate::handlers::Message;

    use super::{filters, models};

    #[tokio::test]
    async fn test_spins_update() {
        let show_db = models::blank_db();
        let spin_db = models::blank_db();
        let connected_users: Arc<Mutex<HashMap<usize, UnboundedSender<Message>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let api = filters::routes(spin_db.clone(), show_db.clone(), connected_users.clone());

        let resp = request()
            .method("POST")
            .path("/spins/update")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .reply(&api)
            .await;

        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_spins_get() {
        let show_db = models::blank_db();
        let spin_db = models::blank_db();
        let connected_users: Arc<Mutex<HashMap<usize, UnboundedSender<Message>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let api = filters::routes(spin_db.clone(), show_db.clone(), connected_users.clone());

        let resp = request().method("GET").path("/spins/get").reply(&api).await;

        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_shows_update() {
        let show_db = models::blank_db();
        let spin_db = models::blank_db();
        let connected_users: Arc<Mutex<HashMap<usize, UnboundedSender<Message>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let api = filters::routes(spin_db.clone(), show_db.clone(), connected_users.clone());

        let resp = request()
            .method("POST")
            .path("/shows/update")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .reply(&api)
            .await;

        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_shows_get() {
        let show_db = models::blank_db();
        let spin_db = models::blank_db();
        let connected_users: Arc<Mutex<HashMap<usize, UnboundedSender<Message>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let api = filters::routes(spin_db.clone(), show_db.clone(), connected_users.clone());

        let resp = request().method("GET").path("/shows/get").reply(&api).await;

        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_health_check() {
        let show_db = models::blank_db();
        let spin_db = models::blank_db();
        let connected_users: Arc<Mutex<HashMap<usize, UnboundedSender<Message>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let api = filters::routes(spin_db.clone(), show_db.clone(), connected_users.clone());

        let resp = request()
            .method("GET")
            .path("/healthCheck")
            .reply(&api)
            .await;

        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_not_found() {
        let show_db = models::blank_db();
        let spin_db = models::blank_db();
        let connected_users: Arc<Mutex<HashMap<usize, UnboundedSender<Message>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let api = filters::routes(spin_db.clone(), show_db.clone(), connected_users.clone());

        let resp = request().method("GET").path("/not-found").reply(&api).await;

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
