#define MyAppName "Eureka 2026 1.0"
#define MyAppVersion "2026.1.0"
#define MyAppPublisher "Eureka Nexus"
#define MyAppExeName "EurekaNexusMiner.exe"

[Setup]
AppId={{D52957D8-474C-4C8C-984F-E11A2EC92310}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName=Eureka 2026 1.0
AppPublisher={#MyAppPublisher}

DefaultDirName={autopf}\Eureka Nexus Miner
DefaultGroupName=Eureka Nexus
DisableProgramGroupPage=yes

OutputDir=..\dist\installer
OutputBaseFilename=Eureka-Nexus-Miner-Setup-2026.1.0

SetupIconFile=..\web\branding\eureka_app_icon.ico
UninstallDisplayIcon={app}\eureka_app_icon.ico

Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern

ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog commandline

CloseApplications=yes
RestartApplications=no

VersionInfoVersion=2026.1.0.0
VersionInfoCompany=Eureka Nexus
VersionInfoDescription=Eureka 2026 1.0
VersionInfoProductName=Eureka 2026 1.0
VersionInfoProductVersion=2026.1.0

[Files]
Source: "..\dist\Eureka-Nexus-Miner-Official-2026.1.0-Windows-x86_64\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "..\web\branding\eureka_app_icon.ico"; DestDir: "{app}"; DestName: "eureka_app_icon.ico"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Eureka 2026 1.0"; Filename: "{app}\{#MyAppExeName}"; WorkingDir: "{app}"; IconFilename: "{app}\eureka_app_icon.ico"
Name: "{autodesktop}\Eureka 2026 1.0"; Filename: "{app}\{#MyAppExeName}"; WorkingDir: "{app}"; IconFilename: "{app}\eureka_app_icon.ico"

[Code]
var
  EngineInstallFailed: Boolean;

function GetCustomSetupExitCode(): Integer;
begin
  Result := 0;
  if EngineInstallFailed then Result := 1;
end;

function KawpowMissing(): Boolean;
begin
  Result :=
    (not FileExists(ExpandConstant('{app}\engines\kawpow\kawpowminer.exe'))) or
    (not FileExists(ExpandConstant('{app}\engines\kawpow\nvrtc64_112_0.dll'))) or
    (not FileExists(ExpandConstant('{app}\engines\kawpow\nvrtc-builtins64_112.dll')));
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  ResultCode: Integer;
begin
  if (CurStep = ssPostInstall) and KawpowMissing() then
  begin
    EngineInstallFailed := True;
    WizardForm.StatusLabel.Caption := 'Installing verified KAWPOW GPU engine (internet required)...';
    if not Exec(ExpandConstant('{sys}\WindowsPowerShell\v1.0\powershell.exe'),
      '-NoProfile -ExecutionPolicy Bypass -File "' + ExpandConstant('{app}\INSTALL_KAWPOW_ENGINE_WINDOWS.ps1') + '"',
      ExpandConstant('{app}'), SW_HIDE, ewWaitUntilTerminated, ResultCode) then
      RaiseException('Cannot start the KAWPOW installer. Run Setup again.');
    if (ResultCode <> 0) or KawpowMissing() then
      RaiseException('KAWPOW installation failed. Check your internet connection and run Setup again.');
    EngineInstallFailed := False;
  end;
end;

[Run]
Filename: "{sys}\WindowsPowerShell\v1.0\powershell.exe"; Parameters: "-NoProfile -WindowStyle Hidden -Command ""Start-Sleep -Seconds 3; Start-Process -FilePath '{app}\{#MyAppExeName}' -WorkingDirectory '{app}'"""; WorkingDir: "{app}"; Description: "Open Eureka 2026 1.0"; Flags: nowait postinstall runhidden

[UninstallRun]
Filename: "{cmd}"; Parameters: "/C taskkill /IM EurekaNexusMiner.exe /T /F"; Flags: runhidden; RunOnceId: "KillEurekaDesktop"
Filename: "{cmd}"; Parameters: "/C taskkill /IM eureka-nexus-miner-official.exe /T /F"; Flags: runhidden; RunOnceId: "KillEurekaBackend"
Filename: "{cmd}"; Parameters: "/C taskkill /IM kawpowminer.exe /T /F"; Flags: runhidden; RunOnceId: "KillKawpow"

[UninstallDelete]
Type: filesandordirs; Name: "{app}\engines\kawpow"
Type: dirifempty; Name: "{app}\engines"
Type: filesandordirs; Name: "{app}\data"
Type: files; Name: "{app}\kawpow-install.log"
