// NetCanv - online collaborative paint canvas
// Copyright 2021-2022, liquidev
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
//--------------------------------------------------------------------------------------------------
//
// Welcome to main.rs! You've come this far, and I'm happy to see you here.
// Here are some points of interest within the codebase:
//
//  - main.rs - handles platform details, such as opening a window and setting up the renderer.
//  - paint_canvas.rs - the infinite paint canvas.
//  - assets.rs - asset loading and color schemes.
//  - config.rs - user configuration.
//  - assets/ - does not contain any code, but rather actual assets, such as fonts and icons.
//  - app/ - contains app states (the lobby and paint UI).
//    - paint/ - contains the painting state. This is where you draw things with friends
//      - actions/ - actions available in the overflow menu
//      - tools/ - tools available in the toolbar on the left
//      - mod.rs - the state UI itself
//    - lobby.rs - the lobby UI
//  - net/ - contains networking-related code (communicating with the relay and other clients).
//  - ui/ - contains NetCanv's bespoke UI framework, as well as all the widgets.
//
// This list may become out of date with time, as the app gets refactored, so feel free to explore,
// and maybe even send a PR if you think something here is wrong.
//
// I hope you enjoy hacking on NetCanv!
//    - liquidev

// Prevent opening a console on Windows if this is a release build.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub extern crate self as netcanv;

use std::fmt::Write;
use std::path::Path;
use std::sync::Arc;

use crate::backend::winit::dpi::{PhysicalPosition, PhysicalSize};
use crate::backend::winit::event::{Event, WindowEvent};
use crate::backend::winit::event_loop::{ControlFlow, EventLoop};
use crate::backend::winit::window::{CursorIcon, WindowBuilder};
use crate::config::WindowConfig;
use crate::logger::Logger;
use crate::net::socket::SocketSystem;
use crate::ui::view::{self, View};
use backend::Backend;
use instant::{Duration, Instant};
use log::{debug, info, warn};
use native_dialog::{MessageDialog, MessageType};
use netcanv_i18n::translate_enum::TranslateEnum;
use netcanv_i18n::{Formatted, Language};
use netcanv_renderer::paws::{vector, Layout};
use nysa::global as bus;

use crate::backend::UiRenderFrame;

#[macro_use]
mod common;
#[macro_use]
mod errors;

mod app;
mod assets;
mod backend;
mod clipboard;
mod color;
mod config;
mod image_coder;
mod keymap;
mod logger;
mod net;
mod paint_canvas;
mod project_file;
mod strings;
mod token;
mod ui;
mod viewport;

use app::*;
use assets::*;
use config::config;
use ui::{Input, Ui};

pub use errors::*;

/// The "inner" main function that does all the work, and can fail.
///
/// `language` is populated with the user's language once that's loaded. The language is then used
/// for displaying crash messages.
async fn inner_main(language: &mut Option<Language>) -> errors::Result<()> {
   // Set up logging.
   Logger::init(Some(Path::new("netcanv.log"))).map_err(|e| Error::CouldNotInitializeLogger {
      error: e.to_string(),
   })?;
   info!("NetCanv {} - welcome!", env!("CARGO_PKG_VERSION"));

   // Load user configuration.
   config::load_or_create()?;

   // Set up the winit event loop and open the window.
   debug!("opening window");
   let event_loop = EventLoop::new();
   let window_builder = {
      let b = WindowBuilder::new()
         .with_inner_size(PhysicalSize::<u32>::new(1024, 600))
         .with_title("NetCanv")
         .with_resizable(true);
      let b = if let Some(window) = &config().window {
         b.with_inner_size(PhysicalSize::new(window.width, window.height))
      } else {
         b
      };

      b
   };

   // Load color scheme.
   // TODO: User-definable color schemes, anyone?
   let color_scheme = ColorScheme::from(config().ui.color_scheme);

   // Build the render backend.
   debug!("initializing render backend");
   let renderer = Backend::new(window_builder, &event_loop).await.map_err(|e| {
      Error::CouldNotInitializeBackend {
         error: e.to_string(),
      }
   })?;
   // Position and maximize the window.
   // NOTE: winit is a bit buggy and WindowBuilder::with_maximized does not
   // make window maximized, but Window::set_maximized does.
   if let Some(window) = &config().window {
      renderer.window().set_outer_position(PhysicalPosition::new(window.x, window.y));
      renderer.window().set_maximized(window.maximized);
   }

   // Build the UI.
   let mut ui = Ui::new(renderer);

   // Load all the assets, and start the first app state.
   debug!("loading assets");
   let assets = Assets::new(ui.render(), color_scheme)?;
   let socket_system = SocketSystem::new();
   *language = Some(assets.language.clone());
   let mut app: Option<Box<dyn AppState>> =
      Some(Box::new(lobby::State::new(assets, Arc::clone(&socket_system))) as _);
   let mut input = Input::new();

   // Initialize the clipboard because we now have a window handle and translation strings.
   match clipboard::init() {
      Ok(_) => (),
      Err(error) => {
         log::error!("failed to initialize clipboard: {:?}", error);
         bus::push(common::Error(error));
      }
   }

   debug!("init done! starting event loop");

   let (mut last_window_size, mut last_window_position) = {
      if let Some(window) = &config().window {
         let size = PhysicalSize::new(window.width, window.height);
         let pos = PhysicalPosition::new(window.x, window.y);
         (size, pos)
      } else {
         let size = ui.window().inner_size();
         let pos = ui.window().outer_position().unwrap_or(PhysicalPosition::default());
         (size, pos)
      }
   };

   event_loop.run(move |event, _, control_flow| {
      *control_flow = ControlFlow::Poll;

      match event {
         Event::WindowEvent { event, .. } => {
            match event {
               // Ignore resize event if window is maximized, and move event if position is lower than 0,
               // because it isn't what we want, when saving window's size and position to config file.
               WindowEvent::Resized(new_size) if !ui.window().is_maximized() => {
                  last_window_size = new_size;
               }
               WindowEvent::Moved(new_position) if new_position.x >= 0 && new_position.y >= 0 => {
                  last_window_position = new_position;
               }
               WindowEvent::CloseRequested => {
                  *control_flow = ControlFlow::Exit;
               }
               _ => {
                  input.process_event(&event);
               }
            }
         }

         Event::MainEventsCleared => {
            let window_size = ui.window().inner_size();
            if let Err(error) = ui.render_frame(|ui| {
               ui.root(
                  vector(window_size.width as f32, window_size.height as f32),
                  Layout::Freeform,
               );
               let mut root_view = View::group_sized(ui);
               view::layout::full_screen(&mut root_view);

               input.set_cursor(CursorIcon::Default);
               app.as_mut().unwrap().process(StateArgs {
                  ui,
                  input: &mut input,
                  root_view,
               });
               app = Some(app.take().unwrap().next_state(ui.render()));
            }) {
               log::error!("render error: {}", error)
            }
            input.finish_frame(ui.window());
         }

         Event::LoopDestroyed => {
            // This is a bit cursed, but works.
            Arc::clone(&socket_system).shutdown();

            let window = ui.window();
            let position = last_window_position;
            let size = last_window_size;
            let maximized = window.is_maximized();
            config::write(|config| {
               config.window = Some(WindowConfig {
                  x: position.x,
                  y: position.y,
                  width: size.width,
                  height: size.height,
                  maximized,
               });
            });
         }

         _ => (),
      }
   });
}

async fn async_main() {
   let mut language = None;
   match inner_main(&mut language).await {
      Ok(()) => (),
      Err(payload) => {
         let mut message = String::new();
         let language = language.unwrap_or_else(|| {
            Assets::load_language(Some("en-US")).expect("English language must be present")
         });
         let _ = write!(
            message,
            "{}",
            Formatted::new(language.clone(), "failure")
               .format()
               .with("message", payload.translate(&language))
               .done(),
         );
         log::error!(
            "inner_main() returned with an Err:\n{}",
            payload.translate(&language)
         );
         MessageDialog::new()
            .set_title("NetCanv - Error")
            .set_text(&message)
            .set_type(MessageType::Error)
            .show_alert()
            .unwrap();
      }
   }
}

fn main() {
   let default_panic_hook = std::panic::take_hook();
   std::panic::set_hook(Box::new(move |panic_info| {
      // Pretty panic messages are only enabled in release mode, as they hinder debugging.
      #[cfg(not(debug_assertions))]
      {
         let mut message = heapless::String::<8192>::new();
         let _ = write!(message, "Oh no! A fatal error occured.\n{}", panic_info);
         let _ = write!(message, "\n\nThis is most definitely a bug, so please file an issue on GitHub. https://github.com/liquidev/netcanv");
         let _ = MessageDialog::new()
            .set_title("NetCanv - Fatal Error")
            .set_text(&message)
            .set_type(MessageType::Error)
            .show_alert();
      }
      default_panic_hook(panic_info);
   }));

   let runtime = tokio::runtime::Builder::new_multi_thread()
      .enable_all()
      .build()
      .expect("cannot start async runtime");

   runtime.block_on(async_main());

   // Don't want the app to hang forever if any background threads don't manage to shut down quickly.
   let shutdown_start = Instant::now();
   runtime.shutdown_timeout(Duration::from_secs(2));
   let shutdown_elapsed = shutdown_start.elapsed();
   if shutdown_elapsed > Duration::from_millis(100) {
      warn!("background tasks took a long time to shut down ({shutdown_elapsed:?}) - perhaps a missing or incomplete Drop?");
   }
}
