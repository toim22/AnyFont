; AnyFont installer (Inno Setup 6.7 or later). Packages target\release\anyfont.exe.
;
;   cargo build --release -p anyfont
;   "%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe" installer\AnyFont.iss      →   dist\AnyFont-<version>-setup.exe
;
; build.bat does both (and the portable .zip). The version comes from the workspace Cargo.toml unless
; /DAppVersion=x.y.z is passed. The wizard pictures and anyfont.ico are drawn from the brand SVG by
; `cargo run -p xtask -- brand`.
;
; Installs for the current user only (no administrator prompt, in %LOCALAPPDATA%\Programs\AnyFont) or, if the user
; chooses, for all users (in Program Files). Settings, the font database and the AnyFont Content folder
; (Documents\AnyFont\Content) live in the user profile and stay there after uninstalling. Setup and the uninstaller
; never install, uninstall or delete fonts: the fonts the user installed with AnyFont stay installed.
;
; The "Product language" page chooses the language AnyFont starts in (one of the 15 languages of AnyFont's
; interface, Windows' display language preselected); the choice is written to Software\AnyFont (Language,
; LanguageStamp) and AnyFont applies it once, at its next start. A silent setup (the in-app update) leaves the
; language alone, and /RELAUNCH=1 starts AnyFont again when it is done:
;
;   AnyFont-<version>-setup.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /RELAUNCH=1

#define AppName      "AnyFont"
#ifndef AppVersion
  ; `version = "x.y.z"` from the [workspace.package] section of Cargo.toml (the INI reader drops the quotes)
  #define AppVersion   ReadIni(SourcePath + "..\Cargo.toml", "workspace.package", "version", "0.0.0")
#endif
#define AppPublisher "AnyFont Team"
#define AppURL       "https://github.com/toim22/AnyFont"
#define AppExe       "anyfont.exe"

[Setup]
; the same AppId in every version: every earlier version is replaced in place (never change it)
AppId={{80A727BB-0AF4-4B46-A41E-269BD512170B}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher={#AppPublisher}
AppPublisherURL={#AppURL}
AppSupportURL={#AppURL}/issues
AppUpdatesURL={#AppURL}/releases
AppCopyright=Copyright © 2026 {#AppPublisher}
VersionInfoVersion={#AppVersion}
VersionInfoCompany={#AppPublisher}
VersionInfoDescription={#AppName} Setup
VersionInfoProductName={#AppName}
VersionInfoProductVersion={#AppVersion}
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
OutputDir=..\dist
OutputBaseFilename=AnyFont-{#AppVersion}-setup
SetupIconFile=..\crates\anyfont\assets\anyfont.ico
UninstallDisplayIcon={app}\{#AppExe}
UninstallDisplayName={#AppName}
Compression=lzma2/max
SolidCompression=yes
; the modern wizard in Windows 11's style, light or dark as Windows is; one picture per display scale
WizardStyle=modern dynamic windows11 hidebevels
DisableWelcomePage=no
WizardImageFile=WizardLarge-100.png,WizardLarge-125.png,WizardLarge-150.png,WizardLarge-175.png,WizardLarge-200.png,WizardLarge-250.png
WizardImageFileDynamicDark=WizardLarge-dark-100.png,WizardLarge-dark-125.png,WizardLarge-dark-150.png,WizardLarge-dark-175.png,WizardLarge-dark-200.png,WizardLarge-dark-250.png
WizardSmallImageFile=WizardSmall-100.png,WizardSmall-125.png,WizardSmall-150.png,WizardSmall-175.png,WizardSmall-200.png,WizardSmall-250.png
WizardSmallImageFileDynamicDark=WizardSmall-100.png,WizardSmall-125.png,WizardSmall-150.png,WizardSmall-175.png,WizardSmall-200.png,WizardSmall-250.png
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.17763
; installs for the current user only (no UAC); the dialog lets the user choose "all users". If the user picks a
; protected folder (such as Program Files) in per-user mode, [Code] offers to restart as administrator
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=commandline dialog
; an update installs where the previous version is, for all users or only this one as before, without asking again
UsePreviousPrivileges=yes
UsePreviousAppDir=yes
UsePreviousTasks=yes
; the installer/uninstaller closes AnyFont if it is running (Restart Manager)
CloseApplications=yes
RestartApplications=no
ChangesAssociations=yes
ShowLanguageDialog=auto

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "brazilianportuguese"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"

[Messages]
english.WelcomeLabel1=Welcome to {#AppName} {#AppVersion}
english.WelcomeLabel2=This will install {#AppName}, the font manager, on your computer.%n%nBrowse, preview, install and organize your fonts: the installed ones and the ones in your folders, with collections, favorites, glyphs and font information at hand.%n%nIt is recommended that you close all other applications before continuing.
english.FinishedHeadingLabel=Ready to set the type
english.FinishedLabel={#AppName} is installed on your computer. Open it from the Start menu.
brazilianportuguese.WelcomeLabel1=Bem-vindo ao {#AppName} {#AppVersion}
brazilianportuguese.WelcomeLabel2=Este assistente instala o {#AppName}, o gerenciador de fontes, no seu computador.%n%nNavegue, visualize, instale e organize suas fontes: as instaladas e as das suas pastas, com coleções, favoritas, glifos e informações das fontes à mão.%n%nRecomenda-se fechar os outros aplicativos antes de continuar.
brazilianportuguese.FinishedHeadingLabel=Letras a postos
brazilianportuguese.FinishedLabel=O {#AppName} está instalado no seu computador. Abra-o pelo menu Iniciar.

[CustomMessages]
english.AppComment=Font manager
english.AssocGroup=File types:
english.AssocFonts=Add {#AppName} to "Open with" for font files (.ttf, .otf, .ttc)
english.FontType=Font file ({#AppName})
english.DirNeedsAdmin=The selected folder requires administrator permission:%n%n%1%n%nTo install there, {#AppName} must be installed for all users. Restart Setup as administrator?
english.DirNotWritable=Setup cannot write to the selected folder:%n%n%1%n%nPlease choose another folder.
english.LangCaption=Product language
english.LangDescription=Choose the language AnyFont starts in.
english.LangSub=AnyFont starts in this language after setup. To choose another language later, run Setup again.
brazilianportuguese.AppComment=Gerenciador de fontes
brazilianportuguese.AssocGroup=Tipos de arquivo:
brazilianportuguese.AssocFonts=Incluir o {#AppName} em "Abrir com" para arquivos de fonte (.ttf, .otf, .ttc)
brazilianportuguese.FontType=Arquivo de fonte ({#AppName})
brazilianportuguese.DirNeedsAdmin=A pasta escolhida exige permissão de administrador:%n%n%1%n%nPara instalar nela, o {#AppName} precisa ser instalado para todos os usuários. Deseja reiniciar a instalação como administrador?
brazilianportuguese.DirNotWritable=Não é possível gravar na pasta escolhida:%n%n%1%n%nEscolha outra pasta.
brazilianportuguese.LangCaption=Idioma do produto
brazilianportuguese.LangDescription=Escolha o idioma em que o AnyFont será iniciado.
brazilianportuguese.LangSub=O AnyFont será iniciado neste idioma após a instalação. Para escolher outro idioma depois, execute a instalação novamente.

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"
; only an "Open with" entry, unchecked: the Windows font viewer stays the default program for font files
Name: "assocfonts"; Description: "{cm:AssocFonts}"; GroupDescription: "{cm:AssocGroup}"; Flags: unchecked

[Files]
Source: "..\target\release\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\crates\anyfont\assets\anyfont.ico"; DestDir: "{app}"; Flags: ignoreversion
; the open-source components AnyFont is built with and their licences (scripts\licenses.ps1)
Source: "..\THIRD-PARTY-NOTICES.txt"; DestDir: "{app}\legal"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"; Comment: "{cm:AppComment}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Comment: "{cm:AppComment}"; Tasks: desktopicon

[Registry]
; the start language chosen on the Product language page (not by a silent setup: an update keeps the user's language).
; Only these two values are removed by the uninstaller; AnyFont's own settings under the key stay.
Root: HKA; Subkey: "Software\AnyFont"; Flags: uninsdeletekeyifempty
Root: HKA; Subkey: "Software\AnyFont"; ValueType: string; ValueName: "Language"; ValueData: "{code:AppLanguage}"; Flags: uninsdeletevalue; Check: not WizardSilent
Root: HKA; Subkey: "Software\AnyFont"; ValueType: string; ValueName: "LanguageStamp"; ValueData: "{code:LanguageStamp}"; Flags: uninsdeletevalue; Check: not WizardSilent
; registered application (gives AnyFont its name in Windows' "Open with" list)
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}"; ValueType: string; ValueName: "FriendlyAppName"; ValueData: "{#AppName}"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}\SupportedTypes"; ValueType: string; ValueName: ".ttf"; ValueData: ""; Tasks: assocfonts
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}\SupportedTypes"; ValueType: string; ValueName: ".otf"; ValueData: ""; Tasks: assocfonts
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}\SupportedTypes"; ValueType: string; ValueName: ".ttc"; ValueData: ""; Tasks: assocfonts
; font files: "Open with" only. The default value of .ttf/.otf/.ttc (the Windows font viewer) is never written.
Root: HKA; Subkey: "Software\Classes\AnyFont.Font"; ValueType: string; ValueName: ""; ValueData: "{cm:FontType}"; Flags: uninsdeletekey; Tasks: assocfonts
Root: HKA; Subkey: "Software\Classes\AnyFont.Font\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\anyfont.ico"; Tasks: assocfonts
Root: HKA; Subkey: "Software\Classes\AnyFont.Font\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""; Tasks: assocfonts
; (the keys go too if the uninstaller leaves them empty, i.e. Setup created them; it uninstalls in reverse order)
Root: HKA; Subkey: "Software\Classes\.ttf"; Flags: uninsdeletekeyifempty; Tasks: assocfonts
Root: HKA; Subkey: "Software\Classes\.otf"; Flags: uninsdeletekeyifempty; Tasks: assocfonts
Root: HKA; Subkey: "Software\Classes\.ttc"; Flags: uninsdeletekeyifempty; Tasks: assocfonts
Root: HKA; Subkey: "Software\Classes\.ttf\OpenWithProgids"; Flags: uninsdeletekeyifempty; Tasks: assocfonts
Root: HKA; Subkey: "Software\Classes\.otf\OpenWithProgids"; Flags: uninsdeletekeyifempty; Tasks: assocfonts
Root: HKA; Subkey: "Software\Classes\.ttc\OpenWithProgids"; Flags: uninsdeletekeyifempty; Tasks: assocfonts
Root: HKA; Subkey: "Software\Classes\.ttf\OpenWithProgids"; ValueType: string; ValueName: "AnyFont.Font"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assocfonts
Root: HKA; Subkey: "Software\Classes\.otf\OpenWithProgids"; ValueType: string; ValueName: "AnyFont.Font"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assocfonts
Root: HKA; Subkey: "Software\Classes\.ttc\OpenWithProgids"; ValueType: string; ValueName: "AnyFont.Font"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assocfonts

[Run]
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent
; the in-app update runs Setup silently with /RELAUNCH=1: AnyFont starts again when it is done
Filename: "{app}\{#AppExe}"; Flags: nowait runasoriginaluser; Check: ShouldRelaunch

; [UninstallDelete] is deliberately empty: the settings, the font database, the Content folder and every font stay.

[Code]
// In "just for me" mode Setup runs without administrator rights; if the user switches to a protected folder (Program
// Files, the root of C:\…), copying would fail with "access denied". When leaving the folder page, Setup checks
// whether it can write there and, if not, offers to restart as administrator (/ALLUSERS) with the chosen folder.
var
  Relaunching: Boolean;
  LangPage: TInputOptionWizardPage;
  LangCodes: TArrayOfString;

// Product language: the language AnyFont starts in, listed as in AnyFont (crates/anyfont/src/i18n.rs, Lang::ALL), by
// its own name. Brazilian Portuguese when the setup runs in Portuguese, else Windows' display language.
procedure AddLang(Code, Name: String);
var
  N: Integer;
begin
  N := GetArrayLength(LangCodes);
  SetArrayLength(LangCodes, N + 1);
  LangCodes[N] := Code;
  LangPage.Add(Name);
end;

// the language of a Windows language id (as Lang::from_langid), English when AnyFont doesn't have it
function LangOfId(Id: Integer): String;
var
  Sub: Integer;
begin
  Sub := Id shr 10;
  case Id and $3FF of
    $04: if (Sub = 1) or (Sub = 3) or (Sub = 5) then Result := 'zh-TW' else Result := 'zh-CN';
    $05: Result := 'cs-CZ';
    $07: Result := 'de-DE';
    $0A: Result := 'es-ES';
    $0C: Result := 'fr-FR';
    $10: Result := 'it-IT';
    $11: Result := 'ja-JP';
    $13: Result := 'nl-NL';
    $15: Result := 'pl-PL';
    $16: Result := 'pt-BR';
    $19: Result := 'ru-RU';
    $1D: Result := 'sv-SE';
    $1F: Result := 'tr-TR';
  else
    Result := 'en-US';
  end;
end;

procedure InitializeWizard;
var
  I: Integer;
  Pick: String;
begin
  LangPage := CreateInputOptionPage(wpSelectTasks, CustomMessage('LangCaption'), CustomMessage('LangDescription'),
    CustomMessage('LangSub'), True, True);
  AddLang('en-US', 'English (United States)');
  AddLang('zh-CN', '中文(简体)');
  AddLang('zh-TW', '中文 (繁體)');
  AddLang('cs-CZ', 'Čeština');
  AddLang('nl-NL', 'Nederlands');
  AddLang('fr-FR', 'Français');
  AddLang('de-DE', 'Deutsch');
  AddLang('it-IT', 'Italiano');
  AddLang('ja-JP', '日本語');
  AddLang('pl-PL', 'Polski');
  AddLang('pt-BR', 'Português Brasileiro');
  AddLang('ru-RU', 'Русский');
  AddLang('es-ES', 'Español');
  AddLang('sv-SE', 'Svenska');
  AddLang('tr-TR', 'Türkçe');
  if ActiveLanguage = 'brazilianportuguese' then
    Pick := 'pt-BR'
  else
    Pick := LangOfId(GetUILanguage);
  LangPage.SelectedValueIndex := 0;
  for I := 0 to GetArrayLength(LangCodes) - 1 do
    if LangCodes[I] = Pick then
      LangPage.SelectedValueIndex := I;
end;

function AppLanguage(Param: String): String;
begin
  Result := LangCodes[LangPage.SelectedValueIndex];
end;

// a new stamp on every interactive setup, so AnyFont applies the choice once
function LanguageStamp(Param: String): String;
begin
  Result := GetDateTimeString('yyyymmddhhnnss', #0, #0);
end;

function ShouldRelaunch: Boolean;
begin
  Result := WizardSilent and (ExpandConstant('{param:RELAUNCH|0}') = '1');
end;

// nearest folder that already exists (the install folder may not exist yet)
function ExistingAncestor(Dir: String): String;
var
  Parent: String;
begin
  Result := RemoveBackslashUnlessRoot(Dir);
  while not DirExists(Result) do
  begin
    Parent := ExtractFileDir(Result);
    if (Parent = '') or (Parent = Result) then
    begin
      Result := '';
      Exit;
    end;
    Result := Parent;
  end;
end;

function CanWriteTo(Dir: String): Boolean;
var
  Probe: String;
begin
  Dir := ExistingAncestor(Dir);
  Result := Dir <> '';
  if not Result then
    Exit;
  Probe := AddBackslash(Dir) + '~anyfont-setup-' + GetDateTimeString('hhnnsszzz', #0, #0) + '.tmp';
  Result := SaveStringToFile(Probe, '', False);
  if Result then
    DeleteFile(Probe);
end;

function NextButtonClick(CurPageID: Integer): Boolean;
var
  Dir: String;
  ErrorCode: Integer;
begin
  Result := True;
  if (CurPageID <> wpSelectDir) or WizardSilent then
    Exit;
  Dir := WizardDirValue;
  if CanWriteTo(Dir) then
    Exit;
  Result := False;
  if IsAdminInstallMode then
  begin
    MsgBox(FmtMessage(CustomMessage('DirNotWritable'), [Dir]), mbError, MB_OK);
    Exit;
  end;
  if MsgBox(FmtMessage(CustomMessage('DirNeedsAdmin'), [Dir]), mbConfirmation, MB_YESNO) <> IDYES then
    Exit;
  if ShellExec('runas', ExpandConstant('{srcexe}'),
               '/ALLUSERS /LANG=' + ActiveLanguage + ' /DIR="' + Dir + '"',
               '', SW_SHOW, ewNoWait, ErrorCode) then
  begin
    Relaunching := True;
    WizardForm.Close;
  end
  else if ErrorCode <> 1223 then  // 1223: the user declined UAC; stay on this page
    MsgBox(SysErrorMessage(ErrorCode), mbError, MB_OK);
end;

// An earlier version installed in the other mode would stay next to this one. Installing for all users, a copy that
// was installed only for this user is removed first (its uninstaller, silently, before the files are copied). The
// uninstaller keeps the user's data and fonts, so nothing but the old program files goes.
const
  UninstallKey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\{80A727BB-0AF4-4B46-A41E-269BD512170B}_is1';

procedure RemoveOtherInstall(Root: Integer);
var
  Cmd: String;
  Code: Integer;
begin
  if not RegQueryStringValue(Root, UninstallKey, 'UninstallString', Cmd) then
    Exit;
  Cmd := RemoveQuotes(Cmd);
  Log('Removing the per-user install of an earlier version: ' + Cmd);
  if not Exec(Cmd, '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART', '', SW_HIDE, ewWaitUntilTerminated, Code) then
    Log('Could not run it: ' + SysErrorMessage(Code));
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if (CurStep = ssInstall) and IsAdminInstallMode then
    RemoveOtherInstall(HKCU);
end;

// closes this instance without asking "Exit Setup?" once the elevated one has opened
procedure CancelButtonClick(CurPageID: Integer; var Cancel, Confirm: Boolean);
begin
  if Relaunching then
    Confirm := False;
end;
