use std::sync::Mutex;

use actix_files::Files;
use actix_web::{
    get, post,
    web::{self, Data},
    App, HttpResponse, HttpServer, Responder,
};
use askama::Template;
use damasc::{parser::statement, statement::Statement};
use damasc::repl::Repl;
use serde::Deserialize;

#[derive(Deserialize)]
struct ReplInput {
    statement: String,
}

#[derive(Template)]
#[template(path = "result.html.j2")]
struct ResultTemplate<'x> {
    repl: &'x ReplInput,
    error: Option<String>,
    output: Option<String>,
}

#[derive(Template)]
#[template(path = "index.html.j2")]
struct HomeTemplate<'x> {
    repl: &'x ReplInput,
}

#[post("/")]
async fn eval(
    repl: web::Form<ReplInput>,
    env_mutex: Data<Mutex<Repl<'_, '_, '_>>>,
) -> impl Responder {
    let mut repl_state = env_mutex.lock().unwrap();

    match statement(&repl.statement) {
        Ok((_, stmt)) => {
            let deny = matches!(&stmt, Statement::UseBag(..) | Statement::Import(..) | Statement::Export(..));

            if deny {
                ResultTemplate {
                    error: Some("This command has been disabled in the web UI".into()),
                    repl: &repl,
                    output: None,
                }
            } else {
                let output = match repl_state.execute(stmt) {
                    Ok(r) => Some(format!("Ok: {r}")),
                    Err(damasc::repl::ReplError::Exit) => None,
                    Err(e) => Some(format!("Error: {e:?}")),
                };
    
                ResultTemplate {
                    error: None,
                    repl: &repl,
                    output,
                }
            }
        }
        Err(e) => ResultTemplate {
            error: Some(e.to_string()),
            repl: &repl,
            output: None,
        },
    }
    .render()
    .map(|s| HttpResponse::Ok().content_type("text/html").body(s))
    .unwrap_or_else(template_error)
}

fn template_error(_: askama::Error) -> HttpResponse {
    HttpResponse::InternalServerError()
        .content_type("text/html")
        .body("Template Error")
}

#[get("/")]
async fn home() -> impl Responder {
    HomeTemplate {
        repl: &ReplInput {
            statement: "".to_owned(),
        },
    }
    .render()
    .map(|s| HttpResponse::Ok().content_type("text/html").body(s))
    .unwrap_or_else(template_error)
}

#[actix_web::main] // or #[tokio::main]
async fn main() -> std::io::Result<()> {
    let mut repl = Repl::new("init");
    let Ok((_, stmt)) = statement(".bag jail as _ limit 30") else {
        return Err(std::io::Error::new(std::io::ErrorKind::Other, "Failed to parse statement"));
    };
    let Ok(_) = repl.execute(stmt) else {
        return Err(std::io::Error::new(std::io::ErrorKind::Other, "Failed to create bag"));
    };
    let repl_mutex = Data::new(Mutex::new(repl));

    let server = HttpServer::new(move || {
        App::new()
            .app_data(repl_mutex.clone())
            .service(home)
            .service(eval)
            .service(Files::new("/", "./static/root/").index_file("index.html"))
    })
    .bind(("127.0.0.1", 8080))?;

    println!("Server started: http://127.0.0.1:8080");

    server.run().await
}
