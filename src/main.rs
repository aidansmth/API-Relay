
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
        .or(health_check())
        .or(not_found())
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

    pub fn health_check() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
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
            warp::http::StatusCode::OK,
        ))
    }

    pub async fn get(db: Db) -> Result<impl warp::Reply, warp::Rejection> {
        let db = db.lock().await;
        let resp = db.clone();
        Ok(warp::reply::json(&resp["items"]))
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
    async fn test_update() {
        let db = models::blank_db();
        let api = filters::routes(db.clone());

        let resp = request()
            .method("POST")
            .path("/update")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .reply(&api)
            .await;

        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_get() {
        let db = models::blank_db();
        let api = filters::routes(db.clone());

        let resp = request()
            .method("GET")
            .path("/get")
            .reply(&api)
            .await;

        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_health_check() {
        let db = models::blank_db();
        let api = filters::routes(db.clone());

        let resp = request()
            .method("GET")
            .path("/healthCheck")
            .reply(&api)
            .await;

        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_not_found() {
        let db = models::blank_db();
        let api = filters::routes(db.clone());

        let resp = request()
            .method("GET")
            .path("/not-found")
            .reply(&api)
            .await;

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }


}