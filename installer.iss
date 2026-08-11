; Inno Setup Script for Forja Language Toolchain
#define MyAppName "Forja"
#define MyAppVersion "0.9.2"
#define MyAppPublisher "Forja Lang"
#define MyAppURL "https://github.com/forja-lang/forja"
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
Compression=lzma2/ultra
SolidCompression=yes
LZMAUseSeparateProcess=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=lowest
ChangesEnvironment=yes
ChangesAssociations=yes
SetupIconFile=forge.ico
UninstallDisplayIcon={app}\{#MyAppExeName}

[Languages]
Name: "spanish"; MessagesFile: "compiler:Languages\Spanish.isl"

[Tasks]
Name: "addtopath"; Description: "Agregar Forja al PATH de usuario (para ejecutar 'forja' desde cualquier consola)"; GroupDescription: "Configuración del Sistema:"; Flags: checkedonce
Name: "assocfa"; Description: "Asociar archivos .fa (código fuente Forja) con el sistema"; GroupDescription: "Asociaciones de Archivos:"; Flags: checkedonce

[Files]
Source: "target\release\forja.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "target\release\forja-lsp.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "target\release\forja-dap.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "target\release\forja-rt.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "stdlib\*.fa"; DestDir: "{app}\stdlib"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "ejemplos\*.fa"; DestDir: "{app}\ejemplos"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "INSTRUCCIONES.md"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "COMANDOS.md"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "LICENSE.md"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist

[Registry]
; PATH environment variable
Root: HKCU; Subkey: "Environment"; ValueType: expandsz; ValueName: "Path"; ValueData: "{olddata};{app}"; Flags: preservestringtype; Tasks: addtopath; Check: NotOnPathYet

; File association for .fa files
Root: HKCU; Subkey: "Software\Classes\.fa"; ValueType: string; ValueName: ""; ValueData: "ForjaSourceFile"; Flags: uninsdeletevalue; Tasks: assocfa
Root: HKCU; Subkey: "Software\Classes\ForjaSourceFile"; ValueType: string; ValueName: ""; ValueData: "Archivo de Código Fuente Forja"; Flags: uninsdeletekey; Tasks: assocfa
Root: HKCU; Subkey: "Software\Classes\ForjaSourceFile\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\{#MyAppExeName},0"; Tasks: assocfa
Root: HKCU; Subkey: "Software\Classes\ForjaSourceFile\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" ""%1"""; Tasks: assocfa

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

procedure RemovePath(AppPath: string);
var
  Path: string;
  P: Integer;
begin
  if RegQueryStringValue(HKCU, 'Environment', 'Path', Path) then
  begin
    P := Pos(';' + AppPath, Path);
    if P > 0 then
    begin
      Delete(Path, P, Length(';' + AppPath));
      RegWriteStringValue(HKCU, 'Environment', 'Path', Path);
    end
    else
    begin
      P := Pos(AppPath + ';', Path);
      if P > 0 then
      begin
        Delete(Path, P, Length(AppPath + ';'));
        RegWriteStringValue(HKCU, 'Environment', 'Path', Path);
      end;
    end;
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
  begin
    RemovePath(ExpandConstant('{app}'));
  end;
end;
