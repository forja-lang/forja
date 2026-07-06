$c = Get-Content src\vm.rs -Raw
$c = $c -replace 'self\.stack\.pop\(\)\.ok_or\(ErrorVM::StackUnderflow\([^)]+\)\)\?', 'self.safe_pop()?'
Set-Content src\vm.rs -Value $c
Write-Host "Done"
