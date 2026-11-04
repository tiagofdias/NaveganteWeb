param([string]$Executable = "$PSScriptRoot/../native/target/debug/navegante-desktop.exe")
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$mode = if ($env:NAVEGANTE_TEST_WAIT_DEVICES -eq '1') { '--demo-wait' } else { '--demo' }
$app = Start-Process -FilePath (Resolve-Path $Executable) -ArgumentList $mode -PassThru
function Window {
    $condition = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ProcessIdProperty, $app.Id)
    [System.Windows.Automation.AutomationElement]::RootElement.FindFirst([System.Windows.Automation.TreeScope]::Children,$condition)
}
function Find-Element([string]$Name) {
    $window = Window
    if ($window) {
        $condition = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty,$Name)
        return $window.FindFirst([System.Windows.Automation.TreeScope]::Descendants,$condition)
    }
}
function Wait-Element([string]$Name) {
    $deadline = [DateTime]::UtcNow.AddSeconds(20)
    do {
        $found = Find-Element $Name
        if ($found) { return $found }
        if ($app.HasExited) { throw "Application exited with $($app.ExitCode)" }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Element not found: $Name"
}
function Invoke-Button([string]$Name) {
    $element = Wait-Element $Name
    $pattern = $element.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
    $pattern.Invoke()
    Start-Sleep -Milliseconds 150
}
try {
    Invoke-Button 'Carregar passe'
    if ($mode -eq '--demo-wait') {
        Wait-Element 'Liga o leitor USB.' | Out-Null
        Wait-Element 'Insere o cartão no leitor.' | Out-Null
    }
    Wait-Element 'O que queres fazer?' | Out-Null
    Wait-Element 'Alex · Cartão demo' | Out-Null
    Wait-Element '12,35 €' | Out-Null
    Invoke-Button 'Adicionar saldo'
    Invoke-Button 'Saldo Zapping'
    Invoke-Button 'Rever carregamento'
    Invoke-Button 'Simular pagamento de 10,00 €'
    Wait-Element 'Pagamento confirmado.' | Out-Null
    $app.CloseMainWindow() | Out-Null
    Wait-Element 'Sair com um pedido em curso?' | Out-Null
    Invoke-Button 'Continuar na aplicação'
    $checkbox = Wait-Element 'O passe da compra está inserido. Confirmo a gravação.'
    $checkbox.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern).Toggle()
    Invoke-Button 'Simular carregamento'
    Wait-Element 'Boa viagem.' | Out-Null
    Invoke-Button 'Voltar ao início'
    Invoke-Button 'Carregar passe'
    Wait-Element '22,35 €' | Out-Null
    # Debug builds have a Windows console host; release builds use the GUI subsystem.
    $children = @(Get-CimInstance Win32_Process -Filter "ParentProcessId = $($app.Id)" | Where-Object Name -ne 'conhost.exe')
    if ($children.Count) { throw 'Native application must not start browser or helper processes' }
    $listeners = @(Get-NetTCPConnection -State Listen -OwningProcess $app.Id -ErrorAction SilentlyContinue)
    if ($listeners.Count) { throw 'Native application must not start a local HTTP server' }
    $app.CloseMainWindow() | Out-Null
    if (-not $app.WaitForExit(5000)) { throw 'Native application did not close' }
    if ($app.ExitCode -ne 0) { throw "Unexpected exit code $($app.ExitCode)" }
    Write-Output 'PASS: native window, automatic reader/card steps, card data, purchase, payment, write confirmation, updated balance, no browser children or local server.'
} finally {
    if (-not $app.HasExited) { $app.Kill() }
}
