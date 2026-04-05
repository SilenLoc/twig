use actix_web::{HttpRequest, HttpResponse, Responder, get, web};
use maud::DOCTYPE;

use crate::auth::{AuthState, extract_api_key};

/// Admin page handler - requires API key
#[get("/admin")]
pub async fn admin_page(req: HttpRequest, auth_state: web::Data<AuthState>) -> impl Responder {
    // Validate API key
    let api_key = match extract_api_key(&req) {
        Some(key) => key,
        None => {
            return HttpResponse::Unauthorized().body("Access denied: Missing API key");
        }
    };

    if !auth_state.validate_api_key(&api_key) {
        return HttpResponse::Unauthorized().body("Access denied: Invalid API key");
    }

    // Fetch data for all tabs
    let tables = auth_state.db.get_all_tables().await.unwrap_or_default();
    let namespaces = auth_state
        .db
        .get_all_namespaces_with_owners()
        .await
        .unwrap_or_default();
    let members = auth_state
        .db
        .get_namespace_members()
        .await
        .unwrap_or_default();
    let users = auth_state.db.get_all_users().await.unwrap_or_default();

    let content = maud::html! {
        h1 class="f3 fw6 white ma0 mb4" { "Admin Dashboard" }

        // Tab navigation
        div class="flex bb b--white-20 mb4" {
            button
                class="pa2 ph3 bg-white-10 white bn br0 pointer hover-bg-white-20 tab-btn active"
                data-tab="tables"
                onclick="showTab('tables')"
            {
                "Database Tables"
            }
            button
                class="pa2 ph3 bg-transparent white bn br0 pointer hover-bg-white-10 tab-btn"
                data-tab="namespaces"
                onclick="showTab('namespaces')"
            {
                "Namespaces"
            }
            button
                class="pa2 ph3 bg-transparent white bn br0 pointer hover-bg-white-10 tab-btn"
                data-tab="users"
                onclick="showTab('users')"
            {
                "Users"
            }
        }

        // Tables Tab
        div id="tables-tab" class="tab-content" {
            h2 class="f4 fw6 white mb3" { "Database Tables" }
            div class="ba b--white-20 br2 bg-black-20 overflow-hidden" {
                div class="flex pa3 bb b--white-20 bg-black-30 white-70 f6 fw6" {
                    div class="flex-auto" { "Table Name" }
                    div class="tr" style="width: 150px;" { "Row Count" }
                }
                @if tables.is_empty() {
                    div class="pa4 tc" {
                        p class="f6 white-70" { "No tables found." }
                    }
                } @else {
                    div class="flex flex-column" {
                        @for (table_name, count) in tables {
                            div class="flex pa3 bb b--white-10 white-90 items-center" {
                                div class="flex-auto f5" {
                                    (table_name)
                                }
                                div class="tr f6" style="width: 150px;" {
                                    span class="ph2 pv1 br1 bg-white-10" {
                                        (count)
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Namespaces Tab
        div id="namespaces-tab" class="tab-content dn" {
            h2 class="f4 fw6 white mb3" { "Namespaces & Access" }

            // Namespaces list with owners
            div class="ba b--white-20 br2 bg-black-20 overflow-hidden mb4" {
                div class="flex pa3 bb b--white-20 bg-black-30 white-70 f6 fw6" {
                    div class="flex-auto" { "Namespace" }
                    div class="tr" style="width: 200px;" { "Owner" }
                }
                @if namespaces.is_empty() {
                    div class="pa4 tc" {
                        p class="f6 white-70" { "No namespaces found." }
                    }
                } @else {
                    div class="flex flex-column" {
                        @for (namespace, owner) in namespaces {
                            div class="flex pa3 bb b--white-10 white-90 items-center" {
                                div class="flex-auto f5" {
                                    (namespace.name)
                                }
                                div class="tr white-60 f6" style="width: 200px;" {
                                    (owner)
                                }
                            }
                        }
                    }
                }
            }

            // Namespace members table
            h3 class="f5 fw6 white mb3" { "Namespace Members" }
            div class="ba b--white-20 br2 bg-black-20 overflow-hidden" {
                div class="flex pa3 bb b--white-20 bg-black-30 white-70 f6 fw6" {
                    div class="flex-25" { "Namespace" }
                    div class="flex-25" { "User" }
                    div class="flex-25" { "Role" }
                    div class="flex-25 tr" { "Added" }
                }
                @if members.is_empty() {
                    div class="pa4 tc" {
                        p class="f6 white-70" { "No namespace members found." }
                    }
                } @else {
                    div class="flex flex-column" {
                        @for (ns_name, username, role, added_at) in members {
                            div class="flex pa3 bb b--white-10 white-90 f6 items-center" {
                                div class="flex-25" { (ns_name) }
                                div class="flex-25 white-80" { (username) }
                                div class="flex-25" {
                                    span class=(format!("ph2 pv1 br1 text-xs {}", role_class(role.as_str()))) {
                                        (role)
                                    }
                                }
                                div class="flex-25 tr white-50 f7" {
                                    (format_timestamp(added_at.as_str()))
                                }
                            }
                        }
                    }
                }
            }
        }

        // Users Tab
        div id="users-tab" class="tab-content dn" {
            h2 class="f4 fw6 white mb3" { "All Users" }
            div class="ba b--white-20 br2 bg-black-20 overflow-hidden" {
                div class="flex pa3 bb b--white-20 bg-black-30 white-70 f6 fw6" {
                    div class="flex-30" { "Username" }
                    div class="flex-40" { "Email" }
                    div class="flex-30 tr" { "Created" }
                }
                @if users.is_empty() {
                    div class="pa4 tc" {
                        p class="f6 white-70" { "No users found." }
                    }
                } @else {
                    div class="flex flex-column" {
                        @for user in users {
                            div class="flex pa3 bb b--white-10 white-90 f6 items-center" {
                                div class="flex-30 fw5" { (user.username) }
                                div class="flex-40 white-80" {
                                    @match &user.email {
                                        Some(email) => (email.as_str()),
                                        None => "-"
                                    }
                                }
                                div class="flex-30 tr white-50 f7" {
                                    (format_timestamp(user.created_at.as_str()))
                                }
                            }
                        }
                    }
                }
            }
        }

        // Tab switching JavaScript
        script {
            "function showTab(tabName) {
                // Hide all tabs
                document.querySelectorAll('.tab-content').forEach(tab => {
                    tab.classList.add('dn');
                });
                // Show selected tab
                document.getElementById(tabName + '-tab').classList.remove('dn');
                // Update button styles
                document.querySelectorAll('.tab-btn').forEach(btn => {
                    btn.classList.remove('bg-white-10');
                    btn.classList.add('bg-transparent');
                });
                document.querySelector('[data-tab=\"' + tabName + '\"]').classList.remove('bg-transparent');
                document.querySelector('[data-tab=\"' + tabName + '\"]').classList.add('bg-white-10');
            }"
        }

        // Additional styles
        style {
            ".flex-25 { flex: 0 0 25%; }
            .flex-30 { flex: 0 0 30%; }
            .flex-40 { flex: 0 0 40%; }
            .text-xs { font-size: 0.75rem; }
            .bg-green-30 { background-color: rgba(0, 255, 0, 0.3); }
            .bg-blue-30 { background-color: rgba(0, 128, 255, 0.3); }
            .bg-yellow-30 { background-color: rgba(255, 193, 7, 0.3); }"
        }
    };

    if req.headers().get("HX-Request").is_some() {
        HttpResponse::Ok()
            .content_type("text/html")
            .body(content.into_string())
    } else {
        HttpResponse::Ok()
            .content_type("text/html")
            .body(render_admin_layout(&content).into_string())
    }
}

fn render_admin_layout(main_content: &maud::Markup) -> maud::Markup {
    maud::html! {
        (DOCTYPE)
        html class="h-100" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "Fig - Admin" }
                link rel="stylesheet" href="/assets/t.css";
                script src="/assets/h.js" {}
                script src="/assets/hx-response-targets.js" {}
            }
            body hx-ext="response-targets" class="w-100 sans-serif ma0 bg-black white" style="height: 100vh; overflow: hidden;" {
                nav class="dt w-100 bg-black bb b--white-20 fixed top-0 left-0 right-0 z-1" {
                    div class="dtc v-mid pa3" {
                        a href="/" class="link white-90 hover-white no-underline fw6 f4" {
                            "Fig"
                        }
                        span class="white-50 ml2" { "/ Admin" }
                    }
                    div class="dtc v-mid tr pa3" {
                        a href="/" class="link white-70 hover-white no-underline f6" {
                            "Back to Home"
                        }
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

fn role_class(role: &str) -> &'static str {
    match role {
        "owner" => "bg-green-30",
        "admin" => "bg-blue-30",
        _ => "bg-yellow-30",
    }
}

fn format_timestamp(ts: &str) -> String {
    // Try to parse and format timestamp nicely
    ts.replace('T', " ")
        .split('.')
        .next()
        .unwrap_or(ts)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_class() {
        assert_eq!(role_class("owner"), "bg-green-30");
        assert_eq!(role_class("admin"), "bg-blue-30");
        assert_eq!(role_class("member"), "bg-yellow-30");
        assert_eq!(role_class("other"), "bg-yellow-30");
    }

    #[test]
    fn test_format_timestamp() {
        let ts = "2024-01-15T10:30:00.000Z";
        let formatted = format_timestamp(ts);
        assert_eq!(formatted, "2024-01-15 10:30:00");
    }
}
