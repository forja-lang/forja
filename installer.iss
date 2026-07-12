; Inno Setup Script for Forja Language Toolchain
#define MyAppName "Forja"
#define MyAppVersion "0.7.0"
#define MyAppPublisher "Forja Lang"
#define MyAppURL "https://github.com/forja-lang"
#define MyAppExeName "forja.exe"

[Setup]
AppId={{D1A2B3C4-E5F6-7A8B-9C0D-1E2F3A4B5C6D}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={autopf}\forja
DisableDirPage=no
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
OutputBaseFilename=forja-installer-x64
Compression=lzma
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=lowest
ChangesEnvironment=yes

[Languages]
Name: "spanish"; MessagesFile: "compiler:Languages\Spanish.isl"

[Files]
Source: "target\release\forja.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "target\release\forja-gui.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "target\release\forja-lsp.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "target\release\forja-dap.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "target\release\forja-rt.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "stdlib\*.fa"; DestDir: "{app}\stdlib"; Flags: ignoreversion recursesubdirs createallsubdirs

[Registry]
Root: HKCU; Subkey: "Environment"; ValueType: expandsz; ValueName: "Path"; ValueData: "{olddata};{app}"; Flags: preservestringtype; Check: NotOnPathYet

[Code]
function NotOnPathYet(): Boolean;
var
  Path: string;
begin
  if RegQueryStringValue(HKCU, 'Environment', 'Path', Path) then
  begin
    Result := Pos(ExpandConstant('{app}'), Path) = 0;
  end
  else
  begin
    Result := True;
  end;
end;
