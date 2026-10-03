#define MyAppName "Eureka Nexus Miner"
#define MyAppVersion "1.1.0"
#define MyAppPublisher "Eureka Nexus"
#define MyAppExeName "EurekaNexusMiner.exe"

[Setup]
AppId={{D52957D8-474C-4C8C-984F-E11A2EC92310}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
AppPublisher={#MyAppPublisher}

DefaultDirName={autopf}\Eureka Nexus Miner
DefaultGroupName=Eureka Nexus
DisableProgramGroupPage=yes

OutputDir=..\dist\installer
OutputBaseFilename=Eureka-Nexus-Miner-Setup-1.1.0

SetupIconFile=..\web\branding\eureka_app_icon.ico
UninstallDisplayIcon={app}\eureka_app_icon.ico

Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern

ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=admin

CloseApplications=yes
RestartApplications=no

VersionInfoVersion=1.1.0.0
VersionInfoCompany=Eureka Nexus
VersionInfoDescription=Eureka Nexus Miner
VersionInfoProductName=Eureka Nexus Miner
VersionInfoProductVersion=1.1.0

[Files]
Source: "..\dist\Eureka-Nexus-Miner-Official-1.1.0-Windows-x86_64\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "..\web\branding\eureka_app_icon.ico"; DestDir: "{app}"; DestName: "eureka_app_icon.ico"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Eureka Nexus Miner"; Filename: "{app}\{#MyAppExeName}"; WorkingDir: "{app}"; IconFilename: "{app}\eureka_app_icon.ico"
Name: "{autodesktop}\Eureka Nexus Miner"; Filename: "{app}\{#MyAppExeName}"; WorkingDir: "{app}"; IconFilename: "{app}\eureka_app_icon.ico"

[Run]
Filename: "{app}\{#MyAppExeName}"; WorkingDir: "{app}"; Description: "Abrir Eureka Nexus Miner"; Flags: nowait postinstall skipifsilent

[UninstallRun]
Filename: "{cmd}"; Parameters: "/C taskkill /IM EurekaNexusMiner.exe /T /F"; Flags: runhidden; RunOnceId: "KillEurekaDesktop"
Filename: "{cmd}"; Parameters: "/C taskkill /IM eureka-nexus-miner-official.exe /T /F"; Flags: runhidden; RunOnceId: "KillEurekaBackend"
Filename: "{cmd}"; Parameters: "/C taskkill /IM kawpowminer.exe /T /F"; Flags: runhidden; RunOnceId: "KillKawpow"
