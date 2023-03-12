#[macro_use]
extern crate log;

use pretty_env_logger;
use tokio_cron_scheduler::{Job, JobScheduler};

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    let spin_db = models::blank_db();
    let show_db = models::blank_db();

    _ = handlers::update_spins(spin_db.clone()).await;
    _ = handlers::update_shows(show_db.clone()).await;

    // Create cron job to update shows every hour
    let _ = create_cron(show_db.clone()).await;

    let api = filters::routes(spin_db, show_db);

    warp::serve(api).run(([127, 0, 0, 1], 8080)).await;
}

async fn create_cron(show_db: models::Db) {
let scheduler = JobScheduler::new().await;

    let show_db_clone = show_db.clone();

    match scheduler {
        Ok(sched) => {
            // create job that refreshes show every 15 minutes
            let job = Job::new_async("0 0,15,30,45 * * * *", move |_, _| {
                let short_lived_db = show_db_clone.clone();
                Box::pin(async {
                    info!("{:?}: updating shows.", chrono::Utc::now());
                    let _ = handlers::update_shows(short_lived_db).await;
                })
            });
            // add job to scheduler
            match job {
                Ok(job) => {
                    match sched.add(job).await {
                        Ok(_) => {
                            info!("Job added to scheduler");
                        }
                        Err(e) => {
                            error!("Error: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("Error: {}", e);
                }
            }
            // start scheduler
            let _ = sched.start().await;
        },
        Err(e) => {
            error!("Error: {}", e);
        }
    }
}

mod filters {
    use super::handlers;
    use super::models::Db;
    use warp::Filter;

    pub fn routes(
        spin_db: Db,
        show_db: Db,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        spin_update(spin_db.clone())
            .or(get_spin(spin_db.clone()))
            .or(show_update(show_db.clone()))
            .or(get_show(show_db.clone()))
            .or(health_check())
            .or(not_found())
    }

    // Update methods
    pub fn spin_update(
        spin_db: Db,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path!("spins" / "update")
            .and(warp::post())
            .and(is_form_content())
            .and(with_db(spin_db))
            .and_then(handlers::update_spins)
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
    }

    pub fn get_show(
        show_db: Db,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path!("shows" / "get")
            .and(warp::get())
            .and(with_db(show_db))
            .and_then(handlers::get)
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

    fn is_form_content() -> impl Filter<Extract = (), Error = warp::Rejection> + Copy {
        warp::header::exact_ignore_case("Content-Type", "application/x-www-form-urlencoded")
    }
}
mod handlers {
    use std::env;

    use log::{debug, info};
    use serde_json::Value;

    use super::models::Db;

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
            new_v[i.to_string()] = new_iter;
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

    pub async fn update_spins(db: Db) -> Result<impl warp::Reply, warp::Rejection> {
        let data_source_url = "https://spinitron.com/api/spins/?access-token=";
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
        // Remove _links field
        *db = remove_links_shows(v).await;

        Ok(warp::reply::with_status(
            "Fetching update.",
            warp::http::StatusCode::OK,
        ))
    }

    pub async fn update_shows(db: Db) -> Result<impl warp::Reply, warp::Rejection> {
        let data_source_url = "https://spinitron.com/api/shows/?access-token=";
        let access_token = match env::var("SPIN_KEY") {
            Ok(v) => v,
            Err(e) => panic!("Couldn't read SPIN_KEY: {}", e),
        };

        let count_url = "&count=2";
        let data_source_url = data_source_url.to_owned() + &access_token + count_url;
        info!("Sending a request to {}", data_source_url);
        let resp = reqwest::get(data_source_url).await.unwrap();

        let str = resp.text().await.unwrap();

        let mut v: Value = serde_json::from_str(&str).unwrap();
        
        let dj1 = v["items"][0]["_links"]["personas"][0]["href"].as_str().unwrap();
        let dj2 = v["items"][1]["_links"]["personas"][0]["href"].as_str().unwrap();

        info!("DJ1: {}", dj1);
        info!("DJ2: {}", dj2);
        
        // Fetch DJ info using reqwest
        let dj1_data = reqwest::get(dj1).await.unwrap().text().await.unwrap();
        let dj2_data = reqwest::get(dj2).await.unwrap().text().await.unwrap();

        let dj1_data: Value = remove_links_djs(serde_json::from_str(&dj1_data).unwrap()).await;
        let dj2_data: Value = remove_links_djs(serde_json::from_str(&dj2_data).unwrap()).await;

        // Remove links

        let mut new_v = remove_links_shows(v).await;

        new_v["dj_0"] = dj1_data;
        new_v["dj_1"] = dj2_data;

        // Store in db
        let mut db = db.lock().await;
        // Remove _links field
        *db = new_v;

        Ok(warp::reply::with_status(
            "Fetching update.",
            warp::http::StatusCode::OK,
        ))
    }

    pub async fn get(db: Db) -> Result<impl warp::Reply, warp::Rejection> {
        let db = db.lock().await;
        let resp = db.clone();
        Ok(warp::reply::json(&resp))
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
    use warp::http::StatusCode;
    use warp::test::request;

    use super::{
        filters,
        models::{self, Db},
    };

    #[tokio::test]
    async fn test_spins_update() {
        let show_db = models::blank_db();
        let spin_db = models::blank_db();
        let api = filters::routes(spin_db.clone(), show_db.clone());

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
        let api = filters::routes(spin_db.clone(), show_db.clone());

        let resp = request().method("GET").path("/spins/get").reply(&api).await;

        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_shows_update() {
        let show_db = models::blank_db();
        let spin_db = models::blank_db();
        let api = filters::routes(spin_db.clone(), show_db.clone());

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
        let api = filters::routes(spin_db.clone(), show_db.clone());

        let resp = request().method("GET").path("/shows/get").reply(&api).await;

        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_health_check() {
        let show_db = models::blank_db();
        let spin_db = models::blank_db();
        let api = filters::routes(spin_db.clone(), show_db.clone());

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
        let api = filters::routes(spin_db.clone(), show_db.clone());

        let resp = request().method("GET").path("/not-found").reply(&api).await;

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
