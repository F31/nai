"""Read installed MSVC/SDK identities; no installer or persistent environment edits."""
import json
import os
from pathlib import Path
import subprocess


def discover_compiler():
    import winreg
    program_files = os.environ.get('ProgramFiles(x86)')
    if not program_files:
        raise RuntimeError('GEMM_NATIVE_COMPILER_UNAVAILABLE')
    vswhere = Path(program_files)/'Microsoft Visual Studio/Installer/vswhere.exe'
    result = subprocess.run([str(vswhere), '-latest', '-prerelease', '-products', '*',
        '-requires', 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64', '-format', 'json', '-utf8'],
        capture_output=True, text=True, encoding='utf-8-sig', timeout=10, check=True)
    installations = json.loads(result.stdout)
    if not installations:
        raise RuntimeError('GEMM_NATIVE_COMPILER_UNAVAILABLE')
    vc = Path(installations[0]['installationPath'])/'VC'
    version = (vc/'Auxiliary/Build/Microsoft.VCToolsVersion.default.txt').read_text().strip()
    tools = vc/'Tools/MSVC'/version
    compiler = tools/'bin/Hostx64/x64/cl.exe'
    sdk = None
    for view in (winreg.KEY_WOW64_32KEY, winreg.KEY_WOW64_64KEY):
        try:
            with winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE,
                    r'SOFTWARE\Microsoft\Windows Kits\Installed Roots', 0, winreg.KEY_READ|view) as key:
                sdk = Path(winreg.QueryValueEx(key, 'KitsRoot10')[0])
                break
        except OSError:
            continue
    if sdk is None:
        raise RuntimeError('GEMM_NATIVE_COMPILER_UNAVAILABLE')
    versions = [p.name for p in (sdk/'Include').iterdir()
        if p.is_dir() and all(x.isdecimal() for x in p.name.split('.'))]
    for sdk_version in sorted(versions, key=lambda v:tuple(map(int,v.split('.'))), reverse=True):
        includes = [tools/'include', *[sdk/'Include'/sdk_version/name for name in ('ucrt','shared','um')]]
        libraries = [tools/'lib/x64', sdk/'Lib'/sdk_version/'ucrt/x64', sdk/'Lib'/sdk_version/'um/x64']
        if compiler.is_file() and all(p.is_dir() for p in [*includes,*libraries]):
            return dict(compiler=str(compiler), includes=list(map(str,includes)),libraries=list(map(str,libraries)),
                environment=dict(VCINSTALLDIR=str(vc), VCToolsVersion=version, WindowsSdkDir=str(sdk),
                    WindowsSDKVersion=sdk_version))
    raise RuntimeError('GEMM_NATIVE_COMPILER_UNAVAILABLE')
