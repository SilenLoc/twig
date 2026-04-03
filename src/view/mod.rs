use actix_web::HttpRequest;
use actix_web::Result as AwResult;
use actix_web::get;
use actix_web::web;
use maud::DOCTYPE;

use crate::config;
use crate::git;
pub mod namespace;
pub mod repo;

#[get("/")]
pub async fn index(req: HttpRequest, server: web::Data<config::Server>) -> AwResult<maud::Markup> {
    // todo handle error
    let namespaces = git::bare::get_namespaces(server.project_root()).unwrap();
    let content = maud::html! {
        @for namespace in namespaces {
            a href=(namespace) {
                (namespace)
            }
        }
    };

    if req.headers().get("HX-Request").is_some() {
        Ok(content)
    } else {
        Ok(render_layout(&content))
    }
}

pub fn render_layout(main_content: &maud::Markup) -> maud::Markup {
    maud::html! {
        (DOCTYPE)
        html class="h-100" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "Fig" }
                link rel="stylesheet" href="/assets/t.css";
                script src="/assets/h.js" {}
            }
            body class="w-100 sans-serif ma0 bg-black white" style="height: 100vh; overflow: hidden;" {
                nav class="dt w-100 bg-black bb b--white-20 fixed top-0 left-0 right-0 z-1" {
                    div class="dtc v-mid pa3" {
                        a href="/" class="link white-90 hover-white no-underline fw6 f4" {
                            "Fig"
                        }
                    }
                    div class="dtc v-mid tr pa3" {

                    }
                }
                main id="feature" class="flex flex-column" style="padding-top: 5rem; padding-left: 10px; padding-right: 10px; padding-bottom: 10px; height: 100vh; overflow: hidden;" {
                    div class="w-100 flex-auto" style="overflow: hidden; display: flex; flex-direction: column;" {
                        (main_content)
                    }
                }
            }
        }
    }
}
