use crate::http::{BUFFER_SIZE, Request, Response, MAX_HEADER_KEY, MAX_HEADER_VALUE, ByteString};
use crate::kv::KeyValueStore;
use crate::template::{process_if, replace};
use crate::user::User;
use crate::include_str_checked;

struct FormErrors {
    user_error: bool,
    password_error: bool,
    password_confirm_error: bool,
    password_match_error: bool,
}

pub fn route_sign_up_post(
    req: &Request,
    resp: &mut Response<MAX_HEADER_KEY, MAX_HEADER_VALUE>,
    mut id_store: &mut KeyValueStore::<u16, u16>,
    mut user_store: &mut KeyValueStore::<u16, User>,
) -> ([u8; BUFFER_SIZE], usize) {
    resp.status = 200;

    if req.method.as_bytes() == b"POST" {
        let entered_username = req.post(b"username");
        let entered_password = req.post(b"password");
        let password_confirmation = req.post(b"password2");
        let mut signup_success = false;

        const TPL_LENGTH: usize = 4096;
        const TPL: &str = include_str_checked!("../../templates/partials/sign-up.html", TPL_LENGTH);
        let mut tpl_bytes: [u8; TPL_LENGTH] = [0; TPL_LENGTH];
        let tpl_slice = TPL.as_bytes();
        if tpl_slice.len() <= tpl_bytes.len() {
            tpl_bytes[..tpl_slice.len()].copy_from_slice(tpl_slice);
        } else {
            resp.write(b"Overflow!");
            return resp.generate();
        }

        match (entered_username, entered_password, password_confirmation) {
            (Some(usr), Some(passwd), Some(passwd2)) if passwd == passwd2 => {
                let mut user = User::new();
                user.id = id_store.get(&0).cloned().unwrap_or(1);
                user.username[..usr.len().min(32)].copy_from_slice(&usr[..usr.len().min(32)]);
                user.password[..passwd.len().min(32)].copy_from_slice(&passwd[..passwd.len().min(32)]);
                user_store.add(user.id, user.clone()).unwrap();

                //-- Increment the next User ID
                id_store.set(0, user.id + 1).unwrap();

                signup_success = true;
            }
            _ => {
                // Initialize error flags
                let mut errors = FormErrors {
                    user_error: false,
                    password_error: false,
                    password_confirm_error: false,
                    password_match_error: false,
                };

                // Determine which errors are present
                match (entered_username, entered_password, password_confirmation) {
                    (None, _, _) => errors.user_error = true,
                    (_, None, _) => errors.password_error = true,
                    (_, Some(password), Some(password2)) if password != password2 => errors.password_match_error = true,
                    (_, _, None) => errors.password_confirm_error = true,
                    _ => (),
                };

                // Handle error scenarios
                let mut user_error = false;
                let mut password_error = false;
                let mut password_match_error = false;
                let mut password_confirm_error = false;
                match (entered_username, entered_password, password_confirmation) {
                    (None, _, _) => user_error = true,
                    (_, None, _) => password_error = true,
                    (_, _, None) => password_confirm_error = true,
                    _ => password_match_error = true,
                };

                if signup_success {
                    process_if(&mut tpl_bytes, signup_success, "{{#if signup_success}}", "{{/if signup_success}}", None, None);
                } else {
                    let error_conditions = [
                        (errors.user_error, "{{#if user_error}}", "{{/if user_error}}"),
                        (errors.password_error, "{{#if password_error}}", "{{/if password_error}}"),
                        (errors.password_confirm_error, "{{#if password_confirm_error}}", "{{/if password_confirm_error}}"),
                        (errors.password_match_error, "{{#if password_match_error}}", "{{/if password_match_error}}"),
                    ];

                    for (condition, if_placeholder, if_end_placeholder) in &error_conditions {
                        process_if(&mut tpl_bytes, *condition, if_placeholder, if_end_placeholder, None, None);
                    }
                }
            }
        }

        //-- Username
        let usr = match entered_username {
            None => {b""}
            Some(usr) => {usr}
        };
        replace(&mut tpl_bytes, "{{username}}", core::str::from_utf8(&usr[..usr.len()]).unwrap_or(""));

        process_if(&mut tpl_bytes,
                   signup_success,
                   "{{#if signup_success}}",
                   "{{/if signup_success}}",
                   Some("{{#else signup_success}}"),
                   Some("{{/else signup_success}}"));

        resp.write(&tpl_bytes[..]);
    } else {
        resp.status = 404;
        resp.headers.append(ByteString::new(b"Content-Type"),  Some(ByteString::new(b"text/html")));
        resp.headers.append(ByteString::new(b"Connection"),  Some(ByteString::new(b"close")));
        return resp.generate();
    }

    //-- Request close connection, do not support keep-alive
    resp.headers.append(ByteString::new(b"Connection"),  Some(ByteString::new(b"close")));
    resp.generate()
}