// Copyright (c) 2021-2023 RBB S.r.l
// opensource@mintlayer.org
// SPDX-License-Identifier: MIT
// Licensed under the MIT License;
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// https://github.com/mintlayer/mintlayer-core/blob/master/LICENSE
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

mod backend;
mod main_window;
mod widgets;

use std::convert::identity;

use backend::messages::{BackendEvent, BackendRequest};
use backend::{node_initialize, BackendControls, BackendSender};
use common::time_getter::TimeGetter;
use iced::widget::{column, container, text};
use iced::Subscription;
use iced::{executor, Application, Command, Element, Length, Settings, Theme};
use iced_aw::native::cupertino::cupertino_spinner::CupertinoSpinner;
use main_window::{MainWindow, MainWindowMessage};
use tokio::sync::mpsc::Receiver;

pub fn main() -> iced::Result {
    MintlayerNodeGUI::run(Settings {
        id: Some("mintlayer-gui".to_owned()),
        antialiasing: true,
        exit_on_close_request: false,
        try_opengles_first: true,
        ..Settings::default()
    })
}

enum MintlayerNodeGUI {
    Loading,
    Loaded(BackendSender, MainWindow),
    IntializationError(String),
}

#[derive(Debug)]
pub enum Message {
    FromBackend(Receiver<BackendEvent>, BackendEvent),
    Loaded(anyhow::Result<BackendControls>),
    EventOccurred(iced::Event),
    ShuttingDownFinished,
    MainWindowMessage(MainWindowMessage),
}

impl Application for MintlayerNodeGUI {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        (
            MintlayerNodeGUI::Loading,
            Command::perform(node_initialize(TimeGetter::default()), Message::Loaded),
        )
    }

    fn title(&self) -> String {
        match self {
            MintlayerNodeGUI::Loading => "Mintlayer Node - Loading...".to_string(),
            MintlayerNodeGUI::Loaded(_backend_sender, w) => {
                format!(
                    "Mintlayer Node - {}",
                    w.node_state().chain_config().chain_type().name()
                )
            }
            MintlayerNodeGUI::IntializationError(_) => "Mintlayer initialization error".to_string(),
        }
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match self {
            MintlayerNodeGUI::Loading => match message {
                Message::FromBackend(_, _) => unreachable!(),
                Message::Loaded(Ok(backend_controls)) => {
                    let BackendControls {
                        initialized_node,
                        backend_sender,
                        backend_receiver,
                    } = backend_controls;
                    *self =
                        MintlayerNodeGUI::Loaded(backend_sender, MainWindow::new(initialized_node));
                    recv_backend_command(backend_receiver)
                }
                Message::Loaded(Err(e)) => {
                    *self = MintlayerNodeGUI::IntializationError(e.to_string());
                    Command::none()
                }
                Message::EventOccurred(event) => {
                    if let iced::Event::Window(iced::window::Event::CloseRequested) = event {
                        panic!("Attempted shutdown during initialization")
                    } else {
                        // While the screen is loading, ignore all events
                        Command::none()
                    }
                }
                Message::ShuttingDownFinished => Command::none(),
                Message::MainWindowMessage(_) => Command::none(),
            },
            MintlayerNodeGUI::Loaded(backend_sender, w) => match message {
                Message::FromBackend(backend_receiver, backend_event) => Command::batch([
                    w.update(
                        MainWindowMessage::FromBackend(backend_event),
                        backend_sender,
                    )
                    .map(Message::MainWindowMessage),
                    recv_backend_command(backend_receiver),
                ]),
                Message::Loaded(_) => unreachable!("Already loaded"),
                Message::EventOccurred(event) => {
                    if let iced::Event::Window(iced::window::Event::CloseRequested) = event {
                        // TODO: this event doesn't cover the case of closing the Window through Cmd+Q in MacOS
                        backend_sender.send(BackendRequest::Shutdown);
                        Command::none()
                    } else {
                        Command::none()
                    }
                }
                Message::ShuttingDownFinished => iced::window::close(),
                Message::MainWindowMessage(msg) => {
                    w.update(msg, backend_sender).map(Message::MainWindowMessage)
                }
            },
            MintlayerNodeGUI::IntializationError(_) => match message {
                Message::FromBackend(_, _) => unreachable!(),
                Message::Loaded(_) => Command::none(),
                Message::EventOccurred(event) => {
                    if let iced::Event::Window(iced::window::Event::CloseRequested) = event {
                        iced::window::close()
                    } else {
                        Command::none()
                    }
                }
                Message::ShuttingDownFinished => iced::window::close(),
                Message::MainWindowMessage(_) => Command::none(),
            },
        }
    }

    fn view(&self) -> Element<Message> {
        match self {
            MintlayerNodeGUI::Loading => {
                container(CupertinoSpinner::new().width(Length::Fill).height(Length::Fill)).into()
            }

            MintlayerNodeGUI::Loaded(_backend_sender, w) => {
                w.view().map(Message::MainWindowMessage)
            }

            MintlayerNodeGUI::IntializationError(e) => {
                let error_box = column![
                    iced::widget::text("Mintlayer-core node initialization failed".to_string())
                        .size(32),
                    iced::widget::text(e.to_string()).size(20),
                    iced::widget::button(text("Close")).on_press(())
                ]
                .align_items(iced::Alignment::Center)
                .spacing(5);

                let res: Element<()> = container(error_box)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x()
                    .center_y()
                    .into();

                res.map(|_| Message::ShuttingDownFinished)
            }
        }
    }

    fn theme(&self) -> Self::Theme {
        Theme::Light
    }

    fn subscription(&self) -> Subscription<Message> {
        iced::subscription::events().map(Message::EventOccurred)
    }
}

fn recv_backend_command(mut backend_receiver: Receiver<BackendEvent>) -> Command<Message> {
    Command::perform(
        async move {
            match backend_receiver.recv().await {
                Some(msg) => Message::FromBackend(backend_receiver, msg),
                None => Message::ShuttingDownFinished,
            }
        },
        identity,
    )
}
