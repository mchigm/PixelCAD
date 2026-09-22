# PixelCAD UI Draft 1 — Design Notes

**Source:** `Pixel CAD UI Draft 1.pdf` (hand-drawn sketch, provided by the
maintainer). A rotated, readable render of the sketch is kept alongside this
file at `docs/design/pixelcad-ui-draft-1.png`.

**Status:** design reference / source of truth for the UI redesign. This
file is the maintainer's own written supplement to the sketch, preserved
verbatim so intent is not lost to chat history. See
`docs/design/ui-redesign-spec.md` (once written) for the synthesized,
implementation-facing spec that maps every item below to "MVP-functional"
or "WIP placeholder."

---

# Linux Window

## Title

- Icon: Icon of PixelCAD, clicking it opens the menu page
- Save Icon: Icon of Save like in Microsoft Paint, clicking it saves / save as (if new file) the file
- Redo Icon: Icon of Redo like in Microsoft Paint, clicking it redos / redos to (RMB click) the user's act
- Undo Icon: Icon of Undo like in Microsoft Paint, clicking it undos / undos to (RMB click) the systems' act
- Filename: The filename opened in PixelCAD
- Settings (Right): Open settings

## Secondary1 (Top)

Tabs:
- Home
- Tool
- View
- Custom

Expand: Other tabs (click to select to show)

Tabs and tab's layout can be customised in settings

##### For the MVP, I think Home tab is enough

'Home' tab features:

- Paste: Icon of Paste like in Microsoft Paint, clicking it pastes [Expand: Paste options]
- Cut (Up, right of paste, above copy): Icon of Cut like in Microsoft Paint, clicking it copies as PNG
- Copy (Low, right of paste, below cut): Icon of Copy like in Microsoft Paint, clicking it copies (if not selected) text the mouse hovering or (if selected) the selected as PNG
- Select: Icon of select like in Microsoft Paint, clicking it enables the selection mode. [Expand: Other select options]
- Default select (Up, right of Select, above Crop & Resize): Icon of Default select (rectangular cropping) like in Microsoft Paint, clicking it enables the rectangular selection tool, which will change cursor to 'aim', than pos1 and pos2 will be clicked via LMB, dotted line will follow the cursor and outline the selected region
- Corp & Resize (Low, right of Select, below Default select): Icon of Corp & resize, clicking it opens menue to apply operations to the selected region
- Stationaries: Includes 'Pen' (Draw), 'Fill', 'Eraser' (Erase with style), 'Text' (Enter text in Canvas) and others. [Expand: Other stationaries as well as other types of 'Pen', 'Fill', 'Eraser', 'Text']
- Brush: Icon of brush like in Microsoft Paint, clicking it enables the brush tool.  [Expand: Choose brush type & style & effect & size]
- Shapes: List of tools like in Microsoft Paint, clicking it selects different tools such as line, eclipse, circle, etc..., supports scrolling or expanding, which shows more tools
- Fill/Outline (Up, right of Shapes, above width): Icon of fill and outline, for 1-D shapes, only 'outline' is supported, for 1.7/2/2.5-D shapes, outline is the style of the lines and fill defines each 'face', note it's difference between the 'outline/fill in other canvas'
- Width (Down, right of Shapes, below Fill/Outline): Icon of width like in Microsoft Paint, clicking it changes the width of fill and outline (while outline line width and density is defiened in Fill/Outline)
- Color & Tools: Color function like in Microsoft Paint, clicking it selects colors and effects
- AI: AI-agent with CV and Editing(drawiing) capabilities

## File

Tabs: avilable canvases (in the same project) or opened files (opened in the APP)

## Main

## Style

Canvas style
- Background: Manage background behaviour
- Backdrop selection: Manage background attribute / select from template
- Texture: Manage background texture
- Export style: Control how image acts after export

Media bay
- Directory tree: List of resources in the project (if applicable)
- Resource finder: List of included resource/file library
- APP storage: List of stored medias in the APP, as well as the 'temp' folder
- Image browser: Download and use images from web or from extensions

New style
- Adds new tabs similar to Canvas style, Media bay

## Secondary2 (Bottom)

##### Note that elements or items are centered

Find
- Elements in the
- Actions made
- Commands
- Edit history
- Conversation history

Command Line
- CLI
- Export/Save
- Modify PATH and directories via simple LINUX commands
- Command: Similar to that in AutoCAD to improve edit efficiency

AI Chat
- Command completion
- Automatic actions

Secondary tools
- Opens toolbox in miniwindow

## Tail

Move: Move icon, clicking it allows drag/move/rotate actions
Click: Cursor icon, clicking it allows select/cursor actions
Canvas size: Manage canvas attributes such as:
- Size
- Format
Zoom/Shrink (Right)

## Vertical tab

Rotate angle: manual roatates canvas with pricision/input angle
Layers

# MacOS Window

##### Since for windowed macOS have the 'cancel/shrink/expand' buttons on the window head, so there's an empty space reserved for it
##### Again, since the design of macOS, the version of windowed and fullscreen will act hugely different, so the following will have 'common' attributes, for both windowed and fullscreen, or attribtues for windowed or fullscreen, size limits will also added as attribute

## Head[Common, hide to title if fullscreen] + Secondary[combine with Head if windowed, display if fullscreen] (Top)

Save
Undo
Redo
Files: File operations [expand: other file operations]

## Vertical tabs (Icons arrange horizontally, when clicked expands vertically tabs)

##### Right-top buttons appear in fullscreen mode, clicking it allows making tab as miniwindow, supports multiple miniwindows

File
- Media bay
- File tree

Main
- Default
- Head from LinuxUI
- Master management

Style
- Style from Linux UI

Secondary
- Secondary 1 from Linux UI [Expand: Choose tab(Linux) selection]

Tool
- Find
- Secondary 2 from Linux UI

Command
- Modify PATH and directories via simple LINUX commands
- Command: Similar to that in AutoCAD to improve edit efficiency

Layers
- Layer settings
- Layers

AI: Agent

## Bottom-right

Zoom/Shrink

##### Note that Linux users can adjust the locations of items via settings, macOS users can only add/reduce items, Windows users can install linux versions via WSL
