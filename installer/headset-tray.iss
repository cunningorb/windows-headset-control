; Per-user installer. No administrator rights, nothing outside HKCU and
; %LOCALAPPDATA%, matching how this project installs by hand.
#define AppName "Headset Tray"
#define AppExe "headset-tray.exe"
#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

[Setup]
; Fixed. Regenerating this makes an upgrade install side by side instead of
; replacing, and strands the old entry in Installed apps.
AppId={{8F2C5A31-7D64-4E19-B0C3-9A5E7F1D2B48}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher=cunningorb
AppSupportURL=https://github.com/cunningorb/windows-headset-control
DefaultDirName={localappdata}\Programs\HeadsetTray
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
DisableDirPage=yes
PrivilegesRequired=lowest
OutputDir=..\dist
OutputBaseFilename=HeadsetTray-{#AppVersion}-setup
SetupIconFile=..\crates\headset-tray\assets\headset.ico
UninstallDisplayIcon={app}\{#AppExe}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
; Close a running tray before replacing its file: Windows forbids overwriting a
; running executable.
CloseApplications=yes
RestartApplications=no

[Tasks]
Name: "startup"; Description: "Start {#AppName} when I sign in"; GroupDescription: "Additional options:"

[Files]
Source: "..\target\release\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\target\release\headsetctl.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
; No IconFilename: the shortcut inherits the executable's own icon resource, so
; there is one icon to keep current rather than two.
Name: "{userprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"; Comment: "Headset settings in the notification area"

[Registry]
; The same value the tray's Settings toggle reads and writes. A Startup-folder
; shortcut would look equivalent and would make that toggle lie.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; \
  ValueName: "HeadsetTray"; ValueData: """{app}\{#AppExe}"""; Tasks: startup; Flags: uninsdeletevalue

[Run]
Filename: "{app}\{#AppExe}"; Description: "Launch {#AppName}"; Flags: postinstall nowait skipifsilent
