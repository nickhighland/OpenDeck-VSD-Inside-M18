use std::path::{Path, PathBuf};

use tiny_http::{Header, Request, Response, Server};

fn mime(extension: &str) -> String {
	match extension {
		"htm" | "html" | "xhtml" => "text/html".to_owned(),
		"js" | "cjs" | "mjs" => "text/javascript".to_owned(),
		"css" => "text/css".to_owned(),
		"png" | "jpeg" | "gif" | "webp" => format!("image/{}", extension),
		"jpg" => "image/jpeg".to_owned(),
		"svg" => "image/svg+xml".to_owned(),
		"json" => "application/json".to_owned(),
		_ => "application/octet-stream".to_owned(),
	}
}

fn header(field: &str, value: &str) -> Option<Header> {
	Header::from_bytes(field.as_bytes(), value.as_bytes()).ok()
}

/// Cross-origin reads are only granted to the app's own local origins. A web
/// page open in the user's browser gets no CORS header and cannot read plugin
/// files or profile data from this server.
fn cors_headers(request: &Request) -> Vec<Header> {
	let origin = request.headers().iter().find(|header| header.field.equiv("Origin")).map(|header| header.value.as_str().to_owned());
	let mut headers = Vec::with_capacity(2);
	if let Some(origin) = origin.filter(|origin| super::is_trusted_origin(origin))
		&& let Some(allow) = header("Access-Control-Allow-Origin", &origin)
	{
		headers.push(allow);
	}
	headers.extend(header("Vary", "Origin"));
	headers
}

fn respond(request: Request, mut response: Response<std::io::Cursor<Vec<u8>>>, headers: Vec<Header>) {
	for header in headers {
		response.add_header(header);
	}
	let _ = request.respond(response);
}

/// Threads answering requests. With several, one slow response (say, to a
/// client that stopped reading) cannot hold up every image behind it.
const WORKERS: usize = 6;

/// Start a simple webserver to serve files of plugins that run in a browser
/// environment, and key images to the editor.
///
/// The server runs on its own threads: `tiny_http` blocks while waiting for
/// requests, and must not occupy the async runtime's worker threads.
pub fn init_webserver(prefix: PathBuf) {
	let spawned = std::thread::Builder::new().name("plugin-webserver".to_owned()).spawn(move || serve(prefix));
	if let Err(error) = spawned {
		log::error!("Failed to start the plugin webserver thread: {error}");
	}
}

fn serve(prefix: PathBuf) {
	let prefix = match prefix.canonicalize() {
		Ok(prefix) => prefix,
		Err(error) => {
			log::error!("Failed to resolve the plugin webserver root {}: {error}", prefix.display());
			return;
		}
	};
	let server = {
		let listener = match std::net::TcpListener::bind((super::LOOPBACK, *super::PORT_BASE + 2)) {
			Ok(listener) => listener,
			Err(error) => {
				log::error!("Failed to bind the plugin webserver: {error}");
				return;
			}
		};

		#[cfg(windows)]
		{
			use std::os::windows::io::AsRawSocket;
			use windows_sys::Win32::Foundation::{HANDLE_FLAG_INHERIT, SetHandleInformation};

			unsafe { SetHandleInformation(listener.as_raw_socket() as _, HANDLE_FLAG_INHERIT, 0) };
		}

		match Server::from_listener(listener, None) {
			Ok(server) => server,
			Err(error) => {
				log::error!("Failed to start the plugin webserver: {error}");
				return;
			}
		}
	};

	let server = std::sync::Arc::new(server);
	for worker in 1..WORKERS {
		let (server, prefix) = (server.clone(), prefix.clone());
		let spawned = std::thread::Builder::new().name(format!("plugin-webserver-{worker}")).spawn(move || {
			for request in server.incoming_requests() {
				handle_request(request, &prefix);
			}
		});
		if let Err(error) = spawned {
			log::warn!("Failed to start a plugin webserver thread: {error}");
		}
	}
	for request in server.incoming_requests() {
		handle_request(request, &prefix);
	}
}

fn handle_request(request: Request, prefix: &Path) {
	// A malformed percent-encoding used to panic here and take the whole
	// webserver (property inspectors, plugin icons) down until restart.
	let Ok(decoded) = urlencoding::decode(request.url()) else {
		let _ = request.respond(Response::empty(400));
		return;
	};
	let mut url = decoded.into_owned();
	if let Some((path, _query)) = url.split_once('?') {
		url = path.to_owned();
	}
	#[cfg(target_os = "windows")]
	let url = url.get(1..).unwrap_or_default().replace('/', "\\");
	let path = Path::new(url.trim_end_matches("|opendeck_property_inspector").trim_end_matches("|opendeck_property_inspector_child"));

	if !matches!(path.try_exists(), Ok(true)) {
		let _ = request.respond(Response::empty(404));
		return;
	}

	// Ensure the requested path is within the config directory to prevent unrestricted access to the filesystem.
	let developer = crate::store::current_settings().developer;
	if !developer && !path.canonicalize().is_ok_and(|p| p.starts_with(prefix)) {
		let _ = request.respond(Response::empty(403));
		return;
	}

	let mut headers = cors_headers(&request);

	// The Svelte frontend cannot call the connectElgatoStreamDeckSocket function on property inspector frames
	// because they are served from a different origin (this webserver on port 57118).
	// Instead, we have to inject a script onto all property inspector frames that receives a message
	// from the Svelte frontend over window.postMessage.

	// Additionally, Tauri cannot support window.open as seperate Tauri windows have seperate JavaScript contexts.
	// However, plugin property inspectors expect access to this function.
	// Instead, we have to inject a replacement window.open implementation that creates an IFrame element
	// and requests the Svelte frontend to maximise the property inspector.

	if let Some(path) = url.strip_suffix("|opendeck_property_inspector") {
		let mut content = std::fs::read_to_string(path).unwrap_or_default();
		content += r#"
			<div id="opendeck_iframe_container" style="position: absolute; z-index: 100; top: 0; left: 0; width: 100%; height: 100%; display: none;"></div>
			<script>
				const opendeck_window_open = window.open;
				const opendeck_iframe_container = document.getElementById("opendeck_iframe_container");

				window.addEventListener("message", (event) => {
					const data = event.data;
					if (data.event == "connect") {
						event.stopImmediatePropagation();
						if (typeof connectOpenActionSocket === "function") connectOpenActionSocket(...data.payload);
						else connectElgatoStreamDeckSocket(...data.payload);
					} else if (data.event == "windowClosed") {
						event.stopImmediatePropagation();
						if (opendeck_iframe_container.firstElementChild) opendeck_iframe_container.firstElementChild.remove();
						opendeck_iframe_container.style.display = "none";
					}
				});

				window.open = (url, target) => {
					if (target && !(target == "_self" || target == "_top")) {
						top.postMessage({ event: "openUrl", payload: url.startsWith("http") ? url : new URL(url, window.location.href).href }, "*");
						return;
					}
					let iframe = document.createElement("iframe");
					iframe.style.flexGrow = "1";
					iframe.onload = () => {
						iframe.contentWindow.opener = window;
						iframe.contentWindow.onbeforeunload = () => top.postMessage({ event: "windowClosed", payload: window.name }, "*");
						iframe.contentWindow.close = () => { iframe.contentWindow.onbeforeunload(); iframe.remove(); };
						iframe.contentWindow.document.body.style.overflowY = "auto";
					};
					iframe.src = url.startsWith("http") ? url : url + "|opendeck_property_inspector_child";
					if (opendeck_iframe_container.firstElementChild) opendeck_iframe_container.firstElementChild.remove();
					opendeck_iframe_container.appendChild(iframe);
					opendeck_iframe_container.style.display = "flex";
					top.postMessage({ event: "windowOpened", payload: window.name }, "*");
					return iframe.contentWindow;
				};

				const opendeck_window_fetch = window.fetch;
				let opendeck_fetch_count = 0;
				let opendeck_fetch_promises = {};
				window.addEventListener("message", (event) => {
					const data = event.data;
					if (data.event == "fetchResponse") {
						event.stopImmediatePropagation();
						const response = new Response(data.payload.response.body, data.payload.response);
						Object.defineProperty(response, "url", { value: data.payload.response.url });
						opendeck_fetch_promises[data.payload.id].resolve(response);
						delete opendeck_fetch_promises[data.payload.id];
					} else if (data.event == "fetchError") {
						event.stopImmediatePropagation();
						opendeck_fetch_promises[data.payload.id].reject(data.payload.error);
						delete opendeck_fetch_promises[data.payload.id];
					}
				});
				window.fetch = (...args) => {
					if (args.length) args[0] = new URL(args[0], window.location.href).href;
					top.postMessage({ event: "fetch", payload: { args, context: window.name, id: ++opendeck_fetch_count }}, "*");
					return new Promise((resolve, reject) => { opendeck_fetch_promises[opendeck_fetch_count] = { resolve, reject }; });
				};
			</script>
		"#;

		headers.extend(header("Content-Type", "text/html"));
		respond(request, Response::from_data(content.into_bytes()), headers);
	} else if let Some(path) = url.strip_suffix("|opendeck_property_inspector_child") {
		let content = std::fs::read_to_string(path).unwrap_or_default();
		let content = format!("<script>window.opener ??= window.parent;</script>{content}");

		headers.extend(header("Content-Type", "text/html"));
		respond(request, Response::from_data(content.into_bytes()), headers);
	} else {
		let mime_type = mime(&match Path::new(&url).extension() {
			Some(extension) => extension.to_string_lossy().to_ascii_lowercase(),
			None => "html".to_owned(),
		});
		headers.extend(header("Content-Type", &mime_type));

		if mime_type.starts_with("text/") || mime_type == "image/svg+xml" || mime_type == "application/json" {
			respond(request, Response::from_data(std::fs::read(&url).unwrap_or_default()), headers);
		} else {
			let file = match std::fs::File::open(&url) {
				Ok(file) => file,
				Err(_) => {
					let _ = request.respond(Response::empty(404));
					return;
				}
			};
			let mut response = Response::from_file(file);
			for header in headers {
				response.add_header(header);
			}
			let _ = request.respond(response);
		}
	}
}

#[cfg(test)]
mod tests {
	use std::io::{Read, Write};
	use std::net::TcpStream;
	use std::time::{Duration, Instant};

	fn request(port: u16, path: &str) -> TcpStream {
		let mut stream = TcpStream::connect((super::super::LOOPBACK, port)).unwrap();
		write!(stream, "GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").unwrap();
		stream
	}

	#[test]
	fn a_client_that_stops_reading_does_not_hold_up_other_requests() {
		let directory = std::env::temp_dir().join(format!("opendeck-webserver-test-{}", std::process::id()));
		std::fs::create_dir_all(&directory).unwrap();
		let large = directory.join("large.png");
		let small = directory.join("small.png");
		// Larger than the loopback socket buffers, so writing it blocks while nobody reads.
		std::fs::write(&large, vec![7u8; 8 * 1024 * 1024]).unwrap();
		std::fs::write(&small, vec![9u8; 1024]).unwrap();
		// There is no app (and so no settings file) in tests.
		crate::store::remember_settings(&crate::store::Settings::default());
		super::init_webserver(directory.clone());
		let port = *super::super::PORT_BASE + 2;
		let started = Instant::now();
		while TcpStream::connect((super::super::LOOPBACK, port)).is_err() && started.elapsed() < Duration::from_secs(5) {
			std::thread::sleep(Duration::from_millis(20));
		}

		let _stalled = request(port, &large.canonicalize().unwrap().to_string_lossy());
		std::thread::sleep(Duration::from_millis(200));

		let mut reader = request(port, &small.canonicalize().unwrap().to_string_lossy());
		reader.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
		let mut response = Vec::new();
		reader.read_to_end(&mut response).expect("the second request was answered");
		assert!(response.starts_with(b"HTTP/1.1 200"), "{}", String::from_utf8_lossy(&response[..response.len().min(80)]));
		assert!(response.ends_with(&[9u8; 1024]));

		let _ = std::fs::remove_dir_all(&directory);
	}
}
