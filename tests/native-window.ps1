param([string]$Executable="$PSScriptRoot/../native/target/debug/navegante-desktop.exe")
$ErrorActionPreference='Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Windows.Forms
Add-Type @'
using System;
using System.Runtime.InteropServices;
public class FullscreenCheck {
 [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left,Top,Right,Bottom; }
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h,out RECT r);
 [DllImport("user32.dll",EntryPoint="GetWindowLongPtrW")] public static extern IntPtr GetStyle(IntPtr h,int index);
}
'@
$app=Start-Process (Resolve-Path $Executable).Path -ArgumentList '--demo' -PassThru
function Window {
 $c=New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ProcessIdProperty,$app.Id)
 [System.Windows.Automation.AutomationElement]::RootElement.FindFirst([System.Windows.Automation.TreeScope]::Children,$c)
}
function Find([string]$name){$w=Window;if($w){$c=New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty,$name);return $w.FindFirst([System.Windows.Automation.TreeScope]::Descendants,$c)}}
function Wait([string]$name){$deadline=[DateTime]::UtcNow.AddSeconds(15);do{$e=Find $name;if($e){return $e};Start-Sleep -Milliseconds 100}while([DateTime]::UtcNow -lt $deadline);throw "Missing element: $name"}
function Click([string]$name){$e=Wait $name;if(-not $e.Current.IsEnabled){throw "Disabled: $name"};$e.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke();Start-Sleep -Milliseconds 200}
function Assert-Fullscreen {
 $handle=[IntPtr](Window).Current.NativeWindowHandle;$r=New-Object FullscreenCheck+RECT
 [FullscreenCheck]::GetWindowRect($handle,[ref]$r)|Out-Null;$bounds=[Windows.Forms.Screen]::FromHandle($handle).Bounds
 if([math]::Abs($r.Left-$bounds.Left)-gt 2 -or [math]::Abs($r.Top-$bounds.Top)-gt 2 -or [math]::Abs($r.Right-$bounds.Right)-gt 2 -or [math]::Abs($r.Bottom-$bounds.Bottom)-gt 2){throw "Window does not fill the complete monitor: window=($($r.Left),$($r.Top),$($r.Right),$($r.Bottom)); screen=$bounds; style=$([FullscreenCheck]::GetStyle($handle,-16).ToInt64())"}
 if(([FullscreenCheck]::GetStyle($handle,-16).ToInt64() -band 0x00C00000)-ne 0){throw 'Fullscreen still has an OS titlebar'}
}
try{
 Wait 'Carregar passe'|Out-Null;Assert-Fullscreen
 Wait 'Voltar ao início'|Out-Null;Wait 'Fechar'|Out-Null
 Click 'Janela';Wait 'Ecrã inteiro'|Out-Null
 $handle=[IntPtr](Window).Current.NativeWindowHandle
 if(([FullscreenCheck]::GetStyle($handle,-16).ToInt64() -band 0x00C00000)-eq 0){throw 'Window mode did not restore its OS frame'}
 Click 'Ecrã inteiro';Wait 'Janela'|Out-Null;Assert-Fullscreen
 Click 'Carregar passe';Wait 'O que queres fazer?'|Out-Null;Click 'Voltar ao início';Wait 'Carregar passe'|Out-Null
 Click 'Carregar passe';Wait 'O que queres fazer?'|Out-Null;Click 'Adicionar saldo';Click 'Saldo Zapping';Click 'Rever carregamento';Click 'Simular pagamento de 10,00 €';Wait 'Pagamento confirmado.'|Out-Null
 Click 'Voltar ao início';Wait 'Tens um carregamento por concluir.'|Out-Null;Click 'Retomar pedido';Wait 'Pagamento confirmado.'|Out-Null
 $e=Wait 'O passe da compra está inserido. Confirmo a gravação.';$e.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern).Toggle()
 Click 'Simular carregamento';Wait 'Boa viagem.'|Out-Null;Click 'Fechar'
 if(-not $app.WaitForExit(5000)){throw 'Close button failed'}
 Write-Output 'PASS: real borderless fullscreen fills monitor; window toggle works; header Home stays visible; pending purchase survives Home/resume; Close works.'
}finally{if(-not $app.HasExited){$app.Kill()}}
