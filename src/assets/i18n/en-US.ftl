## Universal nomenclature

room-id = Room ID

## Lobby

lobby-welcome = Welcome! Host a room or join an existing one to start painting.

lobby-nickname =
   .label = Nickname
   .hint = Name shown to others
lobby-relay-server =
   .label = Relay server
   .hint = Server URL

lobby-join-a-room =
   .title = Join a room
   .description =
      Ask your friend for the { room-id }
      and enter it into the text field below.
lobby-room-id =
   .label = { room-id }
   .hint = 6 characters
lobby-join = Join

lobby-host-a-new-room =
   .title = Host a new room
   .description =
      Create a blank canvas, or load an existing one from file,
      and share the { room-id } with your friends.
lobby-host = Host
lobby-host-from-file = from File

switch-to-dark-mode = Switch to dark mode
switch-to-light-mode = Switch to light mode
open-source-licenses = Open source licenses

connecting = Connecting…

## Paint

paint-welcome-host =
   Welcome to your room!
   To invite friends, send them the { room-id } from the menu in the bottom right corner of your screen.

unknown-host = <unknown>
you-are-the-host = You are the host
someone-is-your-host = is your host
room-id-copied = { room-id } copied to clipboard

someone-joined-the-room = { $nickname } joined the room
someone-left-the-room = { $nickname } has left
someone-is-now-hosting-the-room = { $nickname } is now hosting the room
you-are-now-hosting-the-room = You are now hosting the room

tool-selection = Selection
tool-brush = Brush
tool-eyedropper = Eyedropper

brush-thickness = Thickness

action-save-to-file = Save to file

## File dialogs

fd-supported-image-files = Supported image files
fd-png-file = PNG file
fd-netcanv-canvas = NetCanv canvas

## Color picker

click-to-edit-color = Click to edit color
eraser = Eraser
rgb-hex-code = RGB hex code

## Errors

failure =
   An error occured: { $message }

   If you think this is a bug, please file an issue on GitHub.
   https://github.com/liquidev/netcanv

error = Error: { $error }
error-fatal = Fatal: { $error }

error-could-not-initialize-backend = Could not initialize backend: { $error }
error-could-not-initialize-logger = Could not initialize logger: { $error }
error-translations-do-not-exist = Translations for { $language } do not exist yet.
error-could-not-load-language = Could not load language { $language }. See console log for details

error-nickname-must-not-be-empty = Nickname must not be empty
error-nickname-too-long = The maximum length of a nickname is { $max-length } characters
error-invalid-room-id-length = { room-id } must be a code with { $length } characters
error-while-performing-action = Error while performing action: { $error }
error-while-processing-action = Error while processing action: { $error }
