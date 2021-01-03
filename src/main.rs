extern crate envmnt;
#[macro_use]
extern crate log;
extern crate tokio;

use std::convert::Infallible;
use std::error::Error;
use std::fs::{DirBuilder, File};
use std::io::Write;
use std::net::SocketAddr;
use std::path::Path;

use chrono::{DateTime, offset::Local};
use futures::stream;
use hyper::{Body, Request, Response};
use hyper::{Method, StatusCode};
use hyper::header::CONTENT_TYPE;
use hyper::server::Server;
use hyper::service::{make_service_fn, service_fn};
use multer::Multipart;
use ring::digest;
use serde::Serialize;

#[derive(Serialize)]
struct FileObject {
    filename: String,
    name: String,
    uri: String,
}

impl FileObject {
    fn new(filename: String, name: String, uri: String) -> Self {
        Self {
            filename,
            name,
            uri,
        }
    }

    fn file_hash(&self, content: &[u8]) -> String {
        let digest = digest::digest(&digest::SHA256, content);
        let hash_vec = Vec::from(digest.as_ref());

        let mut hash_str = String::new();
        for i in hash_vec {
            hash_str.push_str(&format!("{:?}", i));
        }

        debug!("{:?}", hash_str);

        hash_str
    }

    fn save_file(&mut self, content: &[u8]) -> Result<(), std::io::Error> {
        let file_hash = self.file_hash(content);
        let file_ext = Path::new(&self.filename).extension().unwrap().to_str().unwrap().to_lowercase();

        let storage_root = envmnt::get_or("STORAGE_ROOT", "./data");
        let path = Path::new(&storage_root);
        let uri = Path::new("");

        let (first_dir, filename) = file_hash.split_at(2);
        let path = path.join(first_dir);
        let uri = uri.join(first_dir);

        let (second_dir, _) = filename.split_at(2);
        let path = path.join(second_dir);
        let uri = uri.join(second_dir);

        DirBuilder::new().recursive(true).create(&path).unwrap();

        let filename = format!("{}.{}", file_hash, file_ext);
        let path = path.join(&filename);
        let uri = uri.join(&filename);
        self.uri = format!("{:?}/{:?}", envmnt::get_or("ACCESS_DOMAIN", "https://ycz0926.site/assets"), uri.to_str().unwrap());

        if path.exists() {
            return Ok(());
        }

        match File::create(&path) {
            Ok(mut f) => {
                f.write_all(content)?;
                Ok(())
            }
            Err(e) => {
                let err = std::io::Error::new(std::io::ErrorKind::Other, e.to_string());
                return Err(err);
            }
        }
    }
}

#[derive(Serialize)]
struct Message {
    msg: String,
    result: Option<Vec<FileObject>>,
}

async fn serve_func(req: Request<Body>) -> Result<Response<Body>, Box<dyn Error + Send + Sync>> {
    let mut response = Response::new(Body::from(""));

    match (req.method(), req.uri().path()) {
        (&Method::POST, "/") => {
            debug!("{:?}", req.headers());

            let boundary = req
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|ct| ct.to_str().ok())
                .and_then(|ct| multer::parse_boundary(ct).ok());
            if boundary.is_none() {
                *response.status_mut() = StatusCode::BAD_REQUEST;
                *response.body_mut() = Body::from("Unsupported content type, multipart/form-data supports only!");
                return Ok(response);
            }

            debug!("{:?}", boundary.clone().unwrap());

            // parse the multipart from request's body
            let full_body = hyper::body::to_bytes(req.into_body()).await.unwrap();
            let body_vec: Vec<Result<Vec<u8>, std::io::Error>> = vec![Ok(full_body.to_vec())];
            let mut multipart = Multipart::new(stream::iter(body_vec), boundary.unwrap());

            let mut result: Vec<FileObject> = vec![];

            while let Some(mut field) = multipart.next_field().await.unwrap() {
                let name = field.name().unwrap().to_owned();
                let filename = field.file_name().unwrap().to_owned();

                debug!("{:?} {:?}", name, filename);

                let mut content = Vec::new();
                while let Some(chunk) = field.chunk().await.unwrap() {
                    content.extend(chunk.to_vec());
                }

                debug!("file size = {:?}", content.len());

                let mut file = FileObject::new(filename, name, "".to_owned());
                match file.save_file(&content) {
                    Ok(_) => {
                        debug!("{:?} save done!", file.uri);
                        result.push(file);
                    }
                    Err(e) => {
                        debug!("something went wrong: {:?}!", e.to_string());
                        file.uri = e.to_string();
                        result.push(file);
                    }
                }
            }

            let message = Message {
                msg: "ok".to_owned(),
                result: Some(result),
            };
            *response.body_mut() = Body::from(serde_json::to_string(&message).unwrap());
        }
        _ => {
            *response.status_mut() = StatusCode::NOT_FOUND;
        }
    };

    Ok(response)
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install CTRL+C signal handler");
}

#[inline]
fn init_storage() {
    let storage_root = envmnt::get_or("STORAGE_ROOT", "./data");
    let path = Path::new(&storage_root);

    if !path.exists() {
        DirBuilder::new().recursive(true).create(envmnt::get_or("STORAGE_ROOT", "./data")).unwrap();
    }

    info!("Storage init done!");
}

#[inline]
fn init_log() {
    let mut builder = env_logger::builder();
    builder.format(|buf, record| {
        let now: DateTime<Local> = Local::now();
        writeln!(buf,
                 "{}",
                 format!(
                     "[{} {} {} line:{}] {}",
                     now.to_string(),
                     record.level().to_string().to_uppercase(),
                     record.module_path().unwrap(),
                     record.line().unwrap(),
                     record.args()))
    });
    builder.init();
}

#[tokio::main]
async fn main() {
    init_log();
    init_storage();

    let addr = SocketAddr::from(([0, 0, 0, 0], 15000));
    let make_svc = make_service_fn(|_conn| async { Ok::<_, Infallible>(service_fn(serve_func)) });
    let server = Server::bind(&addr).serve(make_svc);
    let graceful = server.with_graceful_shutdown(shutdown_signal());

    info!("Server is running!");
    if let Err(e) = graceful.await {
        error!("server error: {}", e.to_string());
    }
}