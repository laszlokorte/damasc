#![cfg(feature = "web")]

use std::env;
use std::io::Error;
use std::sync::Arc;
use std::{collections::BTreeSet, sync::Mutex};

use actix_files::Files;
use actix_web::{
    get,
    http::StatusCode,
    post,
    web::{self, Data},
    App, HttpResponse, HttpServer, Responder,
};
use askama::Template;
use damasc::repl::Repl;
use damasc::{identifier::Identifier, parser::statement, statement::Statement};

use serde::Deserialize;

#[derive(Deserialize)]
struct ReplInput {
    statement: String,
}

#[derive(Template)]
#[template(path = "404.html.j2")]
struct NotFoundTemplate {}

#[derive(Template)]
#[template(path = "result.html.j2")]
struct ResultTemplate<'x> {
    repl: &'x ReplInput,
    error: Option<String>,
    output: Option<String>,
    bags: BTreeSet<Identifier<'x>>,
    vars: BTreeSet<Identifier<'x>>,
}

#[derive(Template)]
#[template(path = "index.html.j2")]
struct HomeTemplate<'x> {
    repl: &'x ReplInput,
}

#[post("/")]
async fn eval(
    repl: web::Form<ReplInput>,
    env_mutex: Data<Arc<Mutex<Repl<'_, '_, '_, '_>>>>,
) -> impl Responder {
    let Ok(mut repl_state) = env_mutex.lock() else {
        return HttpResponse::Ok().content_type("text/html").body("Locked");
    };

    let bags = repl_state.bags();
    let vars = repl_state.vars();

    if repl.statement.len() > 500 {
        return HttpResponse::Ok().content_type("text/html").body(
            ResultTemplate {
                error: Some("Input length is limited to 500 characters".to_string()),
                repl: &repl,
                output: None,
                bags,
                vars,
            }
            .render()
            .unwrap(),
        );
    }

    match statement(&repl.statement) {
        Ok((_, stmt)) => {
            let deny = match &stmt {
                Statement::UseBag(id, ..) => !repl_state.bags().contains(id),
                Statement::Import(..) => true,
                Statement::Export(..) => true,
                _ => false,
            };

            if deny {
                ResultTemplate {
                    error: Some("This command has been disabled in the web UI".into()),
                    repl: &repl,
                    output: None,
                    bags,
                    vars,
                }
            } else {
                let (output, error) = match repl_state.execute(stmt) {
                    Ok(r) => (Some(format!("{r}")), None),
                    Err(damasc::repl::ReplError::Exit) => (None, None),
                    Err(e) => (None, Some(format!("{e:?}"))),
                };

                let bags = repl_state.bags();
                let vars = repl_state.vars();

                ResultTemplate {
                    error,
                    repl: &repl,
                    output,
                    bags,
                    vars,
                }
            }
        }

        Err(e) => ResultTemplate {
            error: Some(e.to_string()),
            repl: &repl,
            output: None,
            bags,
            vars,
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

async fn not_found() -> HttpResponse {
    HttpResponse::build(StatusCode::NOT_FOUND)
        .content_type("text/html; charset=utf-8")
        .body(NotFoundTemplate {}.render().unwrap())
}

#[derive(Deserialize, Debug)]
struct Configuration {
    ip: String,
    port: u16,
}

#[actix_web::main] // or #[tokio::main]
async fn main() -> std::io::Result<()> {
    let mut repl = Repl::new("init");
    let Ok((_, stmt)) = statement(".bag jail as _ limit 30") else {
        return Err(Error::new(std::io::ErrorKind::Other, "Failed to parse statement"));
    };
    let Ok(_) = repl.execute(stmt) else {
        return Err(Error::new(std::io::ErrorKind::Other, "Failed to create bag"));
    };
    let repl_mutex = Arc::new(Mutex::new(Repl::new("init")));
    let repl_mutex_data = Data::new(repl_mutex.clone());

    let conf = Configuration {
        ip: env::var("DAMASC_HOST").unwrap_or("127.0.0.1".into()),
        port: env::var("DAMASC_PORT")
            .ok()
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(8080),
    };

    let server = HttpServer::new(move || {
        App::new()
            .app_data(repl_mutex_data.clone())
            .service(home)
            .service(eval)
            .service(Files::new("/", "./public/"))
            .default_service(web::route().to(not_found))
    })
    .bind((conf.ip, conf.port))?;

    println!("Server started");
    for (adr, scheme) in server.addrs_with_scheme() {
        println!("Listening on {scheme}://{adr}");
    }

    let running = server.run();
    #[cfg(feature = "cli")]
    {
        use futures::try_join;

        try_join!(running, cli(repl_mutex.clone()))?;
        Ok(())
    }

    #[cfg(not(feature = "cli"))]
    {
        return running.await;
    }
}

#[cfg(feature = "cli")]
async fn cli(repl_mutex: Arc<Mutex<Repl<'_, '_, '_, '_>>>) -> Result<(), Error> {
    use damasc::repl::ReplError;
    use rustyline::error::ReadlineError;
    use rustyline::Editor;

    if let Ok(mut rl) = Editor::<()>::new() {
        if rl.load_history("history.txt").is_err() {
            println!("No previous history.");
        }

        println!("Starting REPL because feature 'cli' is enabled.");
        println!("press CTRL-D to exit.");
        println!(".bag");
        if let Ok(repl) = repl_mutex.lock() {
            println!("Current Bag: {}", repl.current_bag);
        };

        loop {
            let readline = rl.readline(">> ");
            match readline {
                Ok(line) => {
                    rl.add_history_entry(line.as_str());
                    let input = line.as_str();

                    let stmt = match statement(input) {
                        Ok((_, s)) => s,
                        Err(e) => {
                            println!("read error: {e}");
                            continue;
                        }
                    };

                    let Ok(mut repl) = repl_mutex.lock() else {
                        continue;
                    };

                    match repl.execute(stmt) {
                        Ok(r) => {
                            println!("{r}")
                        }
                        Err(ReplError::Exit) => {
                            return Err(Error::new(
                                std::io::ErrorKind::BrokenPipe,
                                "Closed by user",
                            ))
                        }
                        Err(e) => println!("Error: {e:?}"),
                    }
                }
                Err(ReadlineError::Interrupted) => {
                    continue;
                }
                Err(ReadlineError::Eof) => {
                    return Err(Error::new(std::io::ErrorKind::BrokenPipe, "Closed by user"))
                }
                Err(err) => {
                    println!("Error: {err}");
                    return Err(Error::new(std::io::ErrorKind::BrokenPipe, err));
                }
            }
        }
    }

    println!("foo");
    Ok(())
}
