// Unreal : « Exécuter sans débogage » de Visual Studio, depuis Island. Compile la cible
// `<Projet>Editor` (Win64 Development) avec UnrealBuildTool, puis ouvre l'éditeur sur le
// .uproject. Le tout dans un onglet de Windows Terminal (on voit la compilation).
//
// Le moteur vient de `EngineAssociation` du .uproject, cherché comme le fait Epic : clé
// HKLM du lanceur, moteurs enregistrés (HKCU…\Builds, par nom ou par dossier `UE_<version>`),
// puis LauncherInstalled.dat. L'extension ne lit pas le registre : c'est le script
// PowerShell lancé dans le terminal qui le fait.

import type { Island } from "@island/sdk";
import type { Project } from "./projects";

const fileName = (p: string) => p.split(/[\\/]/).pop() ?? p;

/** Cible éditeur (`Source/<Nom>Editor.Target.cs`), ou null (projet Blueprint seul). */
export async function editorTarget(island: Island, p: Project): Promise<string | null> {
  const files = await island.files.list(`${p.path}\\Source`).catch(() => []);
  const t = files.find((f) => !f.dir && /Editor\.Target\.cs$/i.test(f.name));
  return t ? t.name.replace(/\.Target\.cs$/i, "") : null;
}

const quote = (s: string) => `'${s.replace(/'/g, "''")}'`;

function script(uproject: string, target: string | null): string {
  return [
    "$ErrorActionPreference='Stop'",
    `$up=${quote(uproject)}`,
    "$a=(Get-Content -Raw $up|ConvertFrom-Json).EngineAssociation",
    "$e=(Get-ItemProperty \"HKLM:\\SOFTWARE\\EpicGames\\Unreal Engine\\$a\" -EA 0).InstalledDirectory",
    "if(!$e){$b=Get-ItemProperty 'HKCU:\\SOFTWARE\\Epic Games\\Unreal Engine\\Builds' -EA 0",
    "if($b){$e=$b.$a;if(!$e){$e=$b.PSObject.Properties|?{$_.Value -match \"UE_$([regex]::Escape($a))[\\\\/]?$\"}|%{$_.Value}|select -f 1}}}",
    "$d=\"$env:ProgramData\\Epic\\UnrealEngineLauncher\\LauncherInstalled.dat\"",
    "if(!$e -and (Test-Path $d)){$e=((gc -Raw $d|ConvertFrom-Json).InstallationList|?{$_.AppName -eq \"UE_$a\"}|select -f 1).InstallLocation}",
    "if(!$e){throw \"Moteur Unreal $a introuvable\"}",
    "Write-Host \"Unreal Engine : $e\" -F Cyan",
    target
      ? `& \"$e\\Engine\\Build\\BatchFiles\\Build.bat\" ${target} Win64 Development \"-Project=$up\" -WaitMutex;if($LASTEXITCODE){throw 'Compilation échouée'}`
      : "",
    "Start-Process \"$e\\Engine\\Binaries\\Win64\\UnrealEditor.exe\" -ArgumentList ('\"'+$up+'\"')",
    "Write-Host 'Éditeur lancé.' -F Green",
  ]
    .filter(Boolean)
    .join("\n");
}

/** PowerShell `-EncodedCommand` : base64 de l'UTF-16LE (aucun guillemet à échapper). */
function encode(src: string): string {
  const bytes = new Uint8Array(src.length * 2);
  for (let i = 0; i < src.length; i++) {
    const c = src.charCodeAt(i);
    bytes[i * 2] = c & 0xff;
    bytes[i * 2 + 1] = c >> 8;
  }
  let bin = "";
  for (const b of bytes) bin += String.fromCharCode(b);
  return btoa(bin);
}

/** Commande « compiler puis lancer l'éditeur ». */
export async function launchCommand(island: Island, p: Project, uproject: string): Promise<{ command: string; title: string }> {
  const target = await editorTarget(island, p);
  return {
    command: `powershell -NoLogo -NoProfile -ExecutionPolicy Bypass -EncodedCommand ${encode(script(uproject, target))}`,
    title: `${fileName(uproject).replace(/\.uproject$/i, "")} · ${target ? "build + éditeur" : "éditeur"}`,
  };
}
