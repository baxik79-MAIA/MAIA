using System;
using System.Collections;
using System.Collections.Generic;
using System.IO;
using System.Net.Sockets;
using System.Runtime.InteropServices;
using System.Security.AccessControl;
using System.Security.Principal;
using System.Security.Cryptography;
using System.Threading;
using System.Text;

internal static class Probe
{
    const uint EXTENDED_STARTUPINFO_PRESENT = 0x00080000;
    const uint CREATE_SUSPENDED = 0x00000004;
    const uint SEM_FAILCRITICALERRORS = 0x0001;
    const uint SEM_NOGPFAULTERRORBOX = 0x0002;
    const uint SEM_NOOPENFILEERRORBOX = 0x8000;
    const int PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES = 0x00020009;
    const int TokenIsAppContainer = 29;
    const int TokenAppContainerSid = 31;
    const int TokenCapabilities = 30;
    const uint TOKEN_QUERY = 0x0008;
    const uint DACL_SECURITY_INFORMATION = 0x00000004;
    const uint GRANT_ACCESS = 1;
    const uint SUB_CONTAINERS_AND_OBJECTS_INHERIT = 0x3;
    const uint FILE_ALL_ACCESS = 0x001F01FF;
    const uint PROCESS_QUERY_LIMITED_INFORMATION = 0x1000;
    const uint WAIT_OBJECT_0 = 0;
    const uint INFINITE = 0xffffffff;

    [StructLayout(LayoutKind.Sequential)] struct SECURITY_CAPABILITIES { public IntPtr AppContainerSid; public IntPtr Capabilities; public uint CapabilityCount; public uint Reserved; }
    [StructLayout(LayoutKind.Sequential)] struct STARTUPINFO { public uint cb; public IntPtr lpReserved, lpDesktop, lpTitle; public uint dwX, dwY, dwXSize, dwYSize, dwXCountChars, dwYCountChars, dwFillAttribute, dwFlags; public short wShowWindow, cbReserved2; public IntPtr lpReserved2, hStdInput, hStdOutput, hStdError; }
    [StructLayout(LayoutKind.Sequential)] struct STARTUPINFOEX { public STARTUPINFO StartupInfo; public IntPtr AttributeList; }
    [StructLayout(LayoutKind.Sequential)] struct PROCESS_INFORMATION { public IntPtr hProcess, hThread; public uint dwProcessId, dwThreadId; }
    [StructLayout(LayoutKind.Sequential)] struct SID_AND_ATTRIBUTES { public IntPtr Sid; public uint Attributes; }
    [StructLayout(LayoutKind.Sequential)] struct IO_COUNTERS { public ulong ReadOperationCount, WriteOperationCount, OtherOperationCount, ReadTransferCount, WriteTransferCount, OtherTransferCount; }
    [StructLayout(LayoutKind.Sequential)] struct BASIC_LIMIT { public long PerProcessUserTimeLimit, PerJobUserTimeLimit; public uint LimitFlags; public UIntPtr MinimumWorkingSetSize, MaximumWorkingSetSize; public uint ActiveProcessLimit; public UIntPtr Affinity; public uint PriorityClass, SchedulingClass; }
    [StructLayout(LayoutKind.Sequential)] struct EXTENDED_LIMIT { public BASIC_LIMIT BasicLimitInformation; public IO_COUNTERS IoInfo; public UIntPtr ProcessMemoryLimit, JobMemoryLimit, PeakProcessMemoryUsed, PeakJobMemoryUsed; }
    [StructLayout(LayoutKind.Sequential)] struct CPU_RATE { public uint ControlFlags, CpuRate; }
    [StructLayout(LayoutKind.Sequential)] struct EXPLICIT_ACCESS { public uint grfAccessPermissions, grfAccessMode, grfInheritance; public TRUSTEE Trustee; }
    [StructLayout(LayoutKind.Sequential)] struct TRUSTEE { public IntPtr pMultipleTrustee; public int MultipleTrusteeOperation, TrusteeForm, TrusteeType; public IntPtr ptstrName; }

    [DllImport("userenv.dll", CharSet=CharSet.Unicode)] static extern int CreateAppContainerProfile(string name, string display, string description, IntPtr capabilities, uint count, out IntPtr sid);
    [DllImport("userenv.dll", CharSet=CharSet.Unicode)] static extern int DeleteAppContainerProfile(string name);
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool InitializeProcThreadAttributeList(IntPtr list, int count, int flags, ref IntPtr size);
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool UpdateProcThreadAttribute(IntPtr list, uint flags, IntPtr attribute, IntPtr value, IntPtr size, IntPtr previous, IntPtr returned);
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)] static extern bool CreateProcess(string app, string command, IntPtr pa, IntPtr ta, bool inherit, uint flags, IntPtr env, string cwd, ref STARTUPINFOEX startup, out PROCESS_INFORMATION info);
    [DllImport("kernel32.dll", SetLastError=true)] static extern uint SetErrorMode(uint mode);
    [DllImport("kernel32.dll", SetLastError=true)] static extern uint ResumeThread(IntPtr thread);
    [DllImport("kernel32.dll", SetLastError=true)] static extern uint WaitForSingleObject(IntPtr handle, uint ms);
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool GetExitCodeProcess(IntPtr process, out uint code);
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool TerminateProcess(IntPtr process, uint code);
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)] static extern IntPtr CreateJobObject(IntPtr attributes, string name);
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool AssignProcessToJobObject(IntPtr job, IntPtr process);
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool IsProcessInJob(IntPtr process, IntPtr job, out bool result);
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool SetInformationJobObject(IntPtr job, int infoClass, IntPtr info, uint length);
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool CloseHandle(IntPtr handle);
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool OpenProcessToken(IntPtr process, uint access, out IntPtr token);
    [DllImport("advapi32.dll", SetLastError=true)] static extern bool GetTokenInformation(IntPtr token, int infoClass, out int info, int length, out int returned);
    [DllImport("advapi32.dll", SetLastError=true)] static extern bool GetTokenInformation(IntPtr token, int infoClass, IntPtr info, int length, out int returned);
    [DllImport("advapi32.dll", SetLastError=true)] static extern uint SetEntriesInAcl(uint count, ref EXPLICIT_ACCESS entries, IntPtr oldAcl, out IntPtr newAcl);
    [DllImport("advapi32.dll", CharSet=CharSet.Unicode, SetLastError=true)] static extern uint SetNamedSecurityInfo(string name, int objectType, uint info, IntPtr owner, IntPtr group, IntPtr dacl, IntPtr sacl);
    [DllImport("kernel32.dll")] static extern IntPtr LocalFree(IntPtr memory);

    static void Fail(string what) { throw new InvalidOperationException(what + "; Win32=" + Marshal.GetLastWin32Error()); }

    [StructLayout(LayoutKind.Sequential)] struct USTR { public ushort Length, MaximumLength; public IntPtr Buffer; }
    [StructLayout(LayoutKind.Sequential)] struct OBJECT_ATTRIBUTES { public uint Length; public IntPtr RootDirectory, ObjectName; public uint Attributes; public IntPtr SecurityDescriptor, SecurityQualityOfService; }
    [StructLayout(LayoutKind.Sequential)] struct IO_STATUS_BLOCK { public IntPtr Status; public UIntPtr Information; }
    [StructLayout(LayoutKind.Sequential)] struct LUID { public uint LowPart; public int HighPart; }
    [StructLayout(LayoutKind.Sequential)] struct LUID_AND_ATTRIBUTES { public LUID Luid; public uint Attributes; }
    [StructLayout(LayoutKind.Sequential)] struct TOKEN_PRIVILEGES { public uint Count; public LUID_AND_ATTRIBUTES First; }
    sealed class NullDaclState { public IntPtr Handle, Dacl, Descriptor; public string OriginalSddl; }
    [DllImport("ntdll.dll")] static extern int NtOpenFile(out IntPtr handle, uint access, ref OBJECT_ATTRIBUTES attributes, out IO_STATUS_BLOCK status, uint share, uint options);
    [DllImport("ntdll.dll")] static extern int NtSetSecurityObject(IntPtr handle,uint info,IntPtr descriptor);
    [DllImport("advapi32.dll", SetLastError=true)] static extern uint GetSecurityInfo(IntPtr handle,uint objectType,uint info,out IntPtr owner,out IntPtr group,out IntPtr dacl,out IntPtr sacl,out IntPtr descriptor);
    [DllImport("advapi32.dll", SetLastError=true)] static extern uint SetSecurityInfo(IntPtr handle,uint objectType,uint info,IntPtr owner,IntPtr group,IntPtr dacl,IntPtr sacl);
    [DllImport("advapi32.dll", SetLastError=true)] static extern bool ConvertSecurityDescriptorToStringSecurityDescriptorW(IntPtr sd,uint revision,uint info,out IntPtr text,out uint length);
    [DllImport("advapi32.dll", CharSet=CharSet.Unicode, SetLastError=true)] static extern bool LookupPrivilegeValue(string system,string name,out LUID luid);
    [DllImport("advapi32.dll", SetLastError=true)] static extern bool AdjustTokenPrivileges(IntPtr token,bool disableAll,ref TOKEN_PRIVILEGES state,uint length,IntPtr previous,IntPtr returned);
    [DllImport("kernel32.dll")] static extern void SetLastError(uint error);
    static void EnableSecurityPrivilege() { IntPtr token; if(!OpenProcessToken(System.Diagnostics.Process.GetCurrentProcess().Handle,TOKEN_QUERY|0x20,out token)) Fail("OpenProcessToken for SeSecurityPrivilege"); try { LUID luid; if(!LookupPrivilegeValue(null,"SeSecurityPrivilege",out luid)) Fail("LookupPrivilegeValue SeSecurityPrivilege"); var p=new TOKEN_PRIVILEGES { Count=1, First=new LUID_AND_ATTRIBUTES { Luid=luid, Attributes=2 } }; SetLastError(0); if(!AdjustTokenPrivileges(token,false,ref p,0,IntPtr.Zero,IntPtr.Zero)) Fail("AdjustTokenPrivileges SeSecurityPrivilege"); int e=Marshal.GetLastWin32Error(); if(e!=0) throw new InvalidOperationException("SeSecurityPrivilege not assigned; Win32="+e); } finally { CloseHandle(token); } }
    static string ReadNullSddl(IntPtr handle,out IntPtr descriptor,out IntPtr dacl) { IntPtr owner,group,sacl; uint r=GetSecurityInfo(handle,1,31,out owner,out group,out dacl,out sacl,out descriptor); if(r!=0) throw new InvalidOperationException("GetSecurityInfo full descriptor; Win32="+r); IntPtr text; uint n; if(!ConvertSecurityDescriptorToStringSecurityDescriptorW(descriptor,1,31,out text,out n)) Fail("Convert full null descriptor"); try { return Marshal.PtrToStringUni(text); } finally { LocalFree(text); } }
    static NullDaclState GrantNullDevice(IntPtr packageSid,string packageSidText) { EnableSecurityPrivilege(); IntPtr name=Marshal.StringToHGlobalUni(@"\Device\Null"); IntPtr nameStruct=Marshal.AllocHGlobal(Marshal.SizeOf(typeof(USTR))); IntPtr handle=IntPtr.Zero,sd=IntPtr.Zero,dacl=IntPtr.Zero,newAcl=IntPtr.Zero,sidBytes=IntPtr.Zero; bool changed=false; try { var u=new USTR { Length=24, MaximumLength=26, Buffer=name }; Marshal.StructureToPtr(u,nameStruct,false); var oa=new OBJECT_ATTRIBUTES { Length=(uint)Marshal.SizeOf(typeof(OBJECT_ATTRIBUTES)), ObjectName=nameStruct, Attributes=0x40 }; IO_STATUS_BLOCK io; int open=NtOpenFile(out handle,0x01060000,ref oa,out io,3,0); if(open<0) throw new InvalidOperationException("NtOpenFile \\Device\\Null WRITE_DAC/READ_CONTROL/ACCESS_SYSTEM_SECURITY; NTSTATUS=0x"+unchecked((uint)open).ToString("X8")); string original=ReadNullSddl(handle,out sd,out dacl); const string expected="O:BAG:SYD:(A;;0x1201bf;;;WD)(A;;FA;;;SY)(A;;FA;;;BA)(A;;0x1200a9;;;RC)S:AI(ML;;NW;;;LW)"; if(original!=expected) throw new InvalidOperationException("Null descriptor drifted from captured baseline; refusing write: "+original); string activePath=Path.Combine(@"C:\MAIA\reports\desk_outbox","m0165_r3_active_restore_"+DateTime.UtcNow.Ticks+".txt"); string body="device=\\Device\\Null"+Environment.NewLine+"package_sid="+packageSidText+Environment.NewLine+"temporary_rights=GENERIC_READ|GENERIC_WRITE"+Environment.NewLine+"original_sddl="+original+Environment.NewLine; Environment.SetEnvironmentVariable("MAIA_R3_RESTORE_RECORD",activePath); using(var fs=new FileStream(activePath,FileMode.CreateNew,FileAccess.Write,FileShare.Read)) { byte[] bytes=Encoding.UTF8.GetBytes(body); fs.Write(bytes,0,bytes.Length); fs.Flush(true); } Console.WriteLine("STANDALONE_RESTORE_RECORD="+activePath); byte[] sidArray=new byte[new SecurityIdentifier(packageSid).BinaryLength]; new SecurityIdentifier(packageSid).GetBinaryForm(sidArray,0); sidBytes=Marshal.AllocHGlobal(sidArray.Length); Marshal.Copy(sidArray,0,sidBytes,sidArray.Length); var ea=new EXPLICIT_ACCESS { grfAccessPermissions=0xC0000000, grfAccessMode=GRANT_ACCESS, grfInheritance=0, Trustee=new TRUSTEE { TrusteeForm=0, TrusteeType=1, ptstrName=sidBytes } }; uint aclStatus=SetEntriesInAcl(1,ref ea,dacl,out newAcl); if(aclStatus!=0) throw new InvalidOperationException("SetEntriesInAcl Package SID; Win32="+aclStatus); uint set=SetSecurityInfo(handle,1,DACL_SECURITY_INFORMATION,IntPtr.Zero,IntPtr.Zero,newAcl,IntPtr.Zero); if(set!=0) throw new InvalidOperationException("SetSecurityInfo DACL Package SID; Win32="+set); changed=true; IntPtr checkSd,checkDacl; string after=ReadNullSddl(handle,out checkSd,out checkDacl); try { if(!after.Contains(packageSidText)) throw new InvalidOperationException("Package SID ACE missing after device DACL update"); Console.WriteLine("TEMPORARY_NULL_ACE=PASS; package_sid="+packageSidText+"; rights=GENERIC_READ|GENERIC_WRITE"); Console.WriteLine("TEMPORARY_SDDL="+after); } finally { LocalFree(checkSd); } return new NullDaclState { Handle=handle,Dacl=dacl,Descriptor=sd,OriginalSddl=original }; } catch { if(changed && handle!=IntPtr.Zero && dacl!=IntPtr.Zero) { int restore=NtSetSecurityObject(handle,DACL_SECURITY_INFORMATION,sd); if(restore<0) Console.Error.WriteLine("EMERGENCY_RESTORE_FAILED NTSTATUS=0x"+unchecked((uint)restore).ToString("X8")); else Console.Error.WriteLine("EMERGENCY_RESTORE_APPLIED"); } if(sd!=IntPtr.Zero) LocalFree(sd); if(handle!=IntPtr.Zero) CloseHandle(handle); throw; } finally { if(newAcl!=IntPtr.Zero) LocalFree(newAcl); if(sidBytes!=IntPtr.Zero) Marshal.FreeHGlobal(sidBytes); Marshal.FreeHGlobal(nameStruct); Marshal.FreeHGlobal(name); } }
    static void RestoreNullDevice(NullDaclState state) { int set=NtSetSecurityObject(state.Handle,DACL_SECURITY_INFORMATION,state.Descriptor); if(set<0) throw new InvalidOperationException("RESTORE_NULL_DACL_FAILED NTSTATUS=0x"+unchecked((uint)set).ToString("X8")); IntPtr sd,dacl; string after=ReadNullSddl(state.Handle,out sd,out dacl); try { if(after!=state.OriginalSddl) throw new InvalidOperationException("RESTORE_DESCRIPTOR_MISMATCH; actual="+after); Console.WriteLine("RESTORE_NULL_DESCRIPTOR=PASS; exact_sddl_match=true; residual_package_sid_ace=false"); } finally { LocalFree(sd); } LocalFree(state.Descriptor); CloseHandle(state.Handle); state.Descriptor=IntPtr.Zero; state.Handle=IntPtr.Zero; }
    static int Main(string[] args)
    {
        uint priorErrorMode = SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX | SEM_NOOPENFILEERRORBOX);
        Console.WriteLine("HOST_ERROR_MODE=0x{0:X8}; prior=0x{1:X8}", SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX | SEM_NOOPENFILEERRORBOX, priorErrorMode);
        if (args.Length > 0 && args[0] == "--child") return Child(args);
        if (args.Length > 0 && args[0] == "--nul-child") return NulChild(args);
        if (args.Length == 0) { Console.Error.WriteLine("usage: Probe.exe <Rust test executable>"); return 2; }
        bool runNulProbe = args[0] == "--nulprobe" || args[0] == "--nulprobe-no-grant";
        bool runSpawnProbe = args[0] == "--spawnprobe";
        bool runCargoDependency = args[0] == "--cargo-dependency";
        bool runBundleNegative = args[0] == "--bundle-negative";
bool runRustfmtPair = args[0] == "--r4-pair";
        string childExe = (runNulProbe || runSpawnProbe || runCargoDependency || runBundleNegative) ? "" : args[0];
        string childArgs = args.Length > 1 ? string.Join(" ", args, 1, args.Length - 1) : "";
        const string depotRoot = @"C:\MAIA\restricted-verifier-depot";
        const string bundleRoot = @"C:\MAIA\restricted-verifier-depot\compat\rustfmt-r5\bundle-9f1ccda00d9cc632";
        const string clippyCompatRoot = @"C:\MAIA\restricted-verifier-depot\compat\clippy-r5v-rust-1.98.1-48a229cea";
        const string vendorRoot = @"C:\MAIA\restricted-verifier-depot\vendor\m0165-r5v-generated-20260928";
        string name = "MAIA.M0165.Probe." + Guid.NewGuid().ToString("N");
        string candidateBase = @"C:\MAIA\candidate-workspaces-r2";
        Directory.CreateDirectory(candidateBase);
        string root = Path.Combine(candidateBase, "maia-m0165-proof-" + Guid.NewGuid().ToString("N"));
        string candidate = Path.Combine(root, "candidate");
        string target = Path.Combine(candidate, "target");
        string protectedDir = Path.Combine(root, "protected");
        Directory.CreateDirectory(target); Directory.CreateDirectory(protectedDir);
        string helloSource = Path.Combine(target, "hello.rs");
        string hostHello = @"C:\MAIA\scratch\m0165-appcontainer-probe\hello.rs";
        if (File.Exists(hostHello)) File.Copy(hostHello, helloSource, true);
        string hostSpawn = @"C:\MAIA\scratch\m0165-appcontainer-probe\spawnprobe.exe";
        if (File.Exists(hostSpawn)) File.Copy(hostSpawn, Path.Combine(target, "spawnprobe.exe"), true);
        string hostFixture = @"C:\MAIA\scratch\m0165-appcontainer-probe\cargo-fixture";
        if (Directory.Exists(hostFixture)) CopyTree(hostFixture, Path.Combine(target, "fixture"));
        string hostDependencyFixture = @"C:\MAIA\scratch\m0165-appcontainer-probe\dependency-fixture";
        if (Directory.Exists(hostDependencyFixture)) CopyTree(hostDependencyFixture, Path.Combine(target, "dependency-fixture"));
        string hostWorkspace = @"C:\MAIA\scratch\m0165-appcontainer-probe\tier1-workspace-r3";
        if (Directory.Exists(hostWorkspace)) CopyTree(hostWorkspace, Path.Combine(target, "workspace"));
        string hostBundleProbe = @"C:\MAIA\scratch\m0165-appcontainer-probe\BundleAclProbe.exe";
        if (File.Exists(hostBundleProbe)) File.Copy(hostBundleProbe, Path.Combine(target, "bundleaclprobe.exe"), true);
        File.Copy(Path.Combine(target, "workspace", "rustfmt.toml"), Path.Combine(candidate, "rustfmt.toml"), true);
        File.WriteAllText(Path.Combine(candidate, "clippy.toml"), "");
        childArgs = childArgs.Replace("{TARGET}", target).Replace("{CONFIG}", Path.Combine(candidate, "rustfmt.toml")).Replace("{DEPOT}", depotRoot);
        if (runBundleNegative) { childExe = Path.Combine(target, "bundleaclprobe.exe"); childArgs = "\"" + Path.Combine(bundleRoot, "rustfmt-r5-compat.exe") + "\""; }
        if (runRustfmtPair) { string src = Path.Combine(target, "workspace", "apps", "local-intelligence-host", "src", "lib.rs"); childExe = Path.Combine(depotRoot, "toolchains", "stable-x86_64-pc-windows-msvc", "bin", "rustfmt.exe"); childArgs = "--check --edition 2024 \"" + src + "\""; }
        if (runRustfmtPair) Console.WriteLine("FMT_CONFIG_PATH=" + Path.Combine(candidate, "rustfmt.toml"));
        if (!String.IsNullOrEmpty(childExe)) childExe = Path.GetFullPath(childExe.Replace("{TARGET}", target));
        bool runBundledRustfmt = childExe.StartsWith(bundleRoot + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase);
        bool useMinimalPath = runBundledRustfmt || runBundleNegative;
        bool runClippyCompat = childArgs.StartsWith("clippy ", StringComparison.OrdinalIgnoreCase);
        string protectedFile = Path.Combine(protectedDir, "sentinel.txt");
        File.WriteAllText(protectedFile, "host-secret-marker");
        IntPtr sid = IntPtr.Zero;
        List<AclSnapshot> aclSnapshots = new List<AclSnapshot>();
        string packageSidText = null;
        NullDaclState nullState = null;
        try
        {
            int hr = CreateAppContainerProfile(name, name, "temporary MAIA M0.16.5 capability probe", IntPtr.Zero, 0, out sid);
            if (hr < 0) { Marshal.ThrowExceptionForHR(hr); }
            SecurityIdentifier packageSid = new SecurityIdentifier(sid);
            packageSidText = packageSid.Value;
            if(args[0] != "--nulprobe-no-grant") nullState = GrantNullDevice(sid, packageSid.Value);
            Grant(candidate, packageSid, FileSystemRights.ReadAndExecute);
            Grant(target, packageSid, FileSystemRights.Modify | FileSystemRights.ReadAndExecute);
            if (runBundleNegative || childExe.StartsWith(bundleRoot + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase))
            {
                GrantTracked(aclSnapshots, bundleRoot, packageSid, FileSystemRights.ReadAndExecute);
            }
            else if (runNulProbe)
            {
                GrantTracked(aclSnapshots, vendorRoot, packageSid, FileSystemRights.ReadAndExecute);
            }
            else if (runSpawnProbe || runCargoDependency || childExe.StartsWith(depotRoot + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase))
            {
                string[] readRoots = new string[] { Path.Combine(depotRoot, "toolchains", "stable-x86_64-pc-windows-msvc"), Path.Combine(depotRoot, "msvc"), Path.Combine(depotRoot, "windows-sdk"), vendorRoot };
                foreach (string readRoot in readRoots) GrantTracked(aclSnapshots, readRoot, packageSid, FileSystemRights.ReadAndExecute);
                if (runClippyCompat) GrantTracked(aclSnapshots, clippyCompatRoot, packageSid, FileSystemRights.ReadAndExecute);
            }
            // Deliberately do not grant the package SID access to protectedDir.
            var listener = new TcpListener(System.Net.IPAddress.Loopback, 0); listener.Start();
            int port = ((System.Net.IPEndPoint)listener.LocalEndpoint).Port;
            string result = Path.Combine(target, "probe.result");
            if (runNulProbe) { string self = System.Diagnostics.Process.GetCurrentProcess().MainModule.FileName; childExe = Path.Combine(target, "LadderR2.exe"); File.Copy(self, childExe, true); childArgs = "--nul-child \"" + result + "\""; }
            if (runSpawnProbe) { childExe=Path.Combine(target,"spawnprobe.exe"); childArgs=String.Join(" ",args,1,args.Length-1); }
            if (runCargoDependency) { childExe=Path.Combine(depotRoot,"toolchains","stable-x86_64-pc-windows-msvc","bin","cargo.exe"); childArgs="test --manifest-path " + Path.Combine(target,"dependency-fixture","Cargo.toml") + " --locked --offline"; }
            string command = "\"" + childExe + "\" " + childArgs;
            string systemRoot = Environment.GetEnvironmentVariable("SYSTEMROOT");
            var variables = new SortedDictionary<string, string>(StringComparer.OrdinalIgnoreCase);
            variables["SYSTEMROOT"] = Environment.GetEnvironmentVariable("SYSTEMROOT");
            variables["WINDIR"] = Environment.GetEnvironmentVariable("WINDIR");
            variables["SystemDrive"] = Environment.GetEnvironmentVariable("SystemDrive");
            variables["=C:"] = Environment.CurrentDirectory;
            variables["USERPROFILE"] = Environment.GetEnvironmentVariable("USERPROFILE");
            variables["HOMEDRIVE"] = Environment.GetEnvironmentVariable("HOMEDRIVE");
            variables["HOMEPATH"] = Environment.GetEnvironmentVariable("HOMEPATH");
            variables["OS"] = Environment.GetEnvironmentVariable("OS");
            variables["PROCESSOR_ARCHITECTURE"] = Environment.GetEnvironmentVariable("PROCESSOR_ARCHITECTURE");
            variables["ComSpec"] = Environment.GetEnvironmentVariable("ComSpec");
            string[] windowsEnv = { "ALLUSERSPROFILE", "APPDATA", "LOCALAPPDATA", "PUBLIC", "ProgramData", "ProgramFiles", "ProgramFiles(x86)", "ProgramW6432", "CommonProgramFiles", "CommonProgramFiles(x86)", "CommonProgramW6432", "COMPUTERNAME", "USERNAME", "USERDOMAIN", "USERDOMAIN_ROAMINGPROFILE", "SESSIONNAME", "NUMBER_OF_PROCESSORS", "PROCESSOR_IDENTIFIER", "PROCESSOR_LEVEL", "PROCESSOR_REVISION", "PATHEXT", "LOGONSERVER" };
            foreach(string key in windowsEnv) { string value=Environment.GetEnvironmentVariable(key); if(value!=null) variables[key]=value; }
            variables["MAIA_APPCONTAINER_CANDIDATE"] = target;
            variables["MAIA_VERIFIER_VENDOR_ROOT"] = vendorRoot;
            variables["MAIA_APPCONTAINER_WORKSPACE"] = candidate;
            variables["MAIA_APPCONTAINER_LOOPBACK_PORT"] = port.ToString();
            variables["MAIA_APPCONTAINER_PROTECTED_FILE"] = "C:\\MAIA\\public-export\\.local\\m0160-supervisor\\README.md";
            variables["MAIA_APPCONTAINER_RESULT_FILE"] = result;
            variables["MAIA_CONTAINED_TEST_MODE"] = "appcontainer-probe";
            variables["MAIA_R3_RESTORE_RECORD"] = Environment.GetEnvironmentVariable("MAIA_R3_RESTORE_RECORD") ?? "";
            variables["TEMP"] = target;
            variables["TMP"] = target;
            string msvcBin = Path.Combine(depotRoot, "msvc", "bin", "Hostx64", "x64");
            string msvcLib = Path.Combine(depotRoot, "msvc", "lib", "x64");
            string sdkUm = Path.Combine(depotRoot, "windows-sdk", "um", "x64");
            string sdkUcrt = Path.Combine(depotRoot, "windows-sdk", "ucrt", "x64");
            variables["PATH"] = useMinimalPath ? Environment.SystemDirectory : Path.Combine(depotRoot, "toolchains", "stable-x86_64-pc-windows-msvc", "bin") + ";" + msvcBin + ";" + Environment.SystemDirectory;
            if (runClippyCompat) variables["PATH"] = clippyCompatRoot + ";" + variables["PATH"];
            variables["LIB"] = msvcLib + ";" + sdkUm + ";" + sdkUcrt;
            variables["INCLUDE"] = Path.Combine(depotRoot, "msvc", "include") + ";" + Path.Combine(depotRoot, "windows-sdk", "include", "ucrt") + ";" + Path.Combine(depotRoot, "windows-sdk", "include", "um") + ";" + Path.Combine(depotRoot, "windows-sdk", "include", "shared");
            variables["CARGO_HOME"] = Path.Combine(target, "cargo-home");
            if (!useMinimalPath)
            {
                variables["RUSTUP_HOME"] = depotRoot;
                variables["RUSTUP_TOOLCHAIN"] = "stable-x86_64-pc-windows-msvc";
                variables["RUSTUP_NO_UPDATE_CHECK"] = "1";
            }
            variables["HOME"] = target;
            variables["CLIPPY_CONF_DIR"] = candidate;
            if (runClippyCompat) variables["SYSROOT"] = Path.Combine(depotRoot, "toolchains", "stable-x86_64-pc-windows-msvc");
            Directory.CreateDirectory(variables["CARGO_HOME"]); string cfg="[source.crates-io]\nreplace-with = \"vendored-sources\"\n\n[source.vendored-sources]\ndirectory = \"C:/MAIA/restricted-verifier-depot/vendor/m0165-r5v-generated-20260928\"\n"; string cargoConfigPath=Path.Combine(variables["CARGO_HOME"],"config.toml"); File.WriteAllText(cargoConfigPath,cfg); var configSecurity=new FileInfo(cargoConfigPath).GetAccessControl(AccessControlSections.Access); configSecurity.SetAccessRuleProtection(true,false); configSecurity.AddAccessRule(new FileSystemAccessRule(new SecurityIdentifier(WellKnownSidType.BuiltinAdministratorsSid,null),FileSystemRights.FullControl,AccessControlType.Allow)); configSecurity.AddAccessRule(new FileSystemAccessRule(new SecurityIdentifier(WellKnownSidType.LocalSystemSid,null),FileSystemRights.FullControl,AccessControlType.Allow)); configSecurity.AddAccessRule(new FileSystemAccessRule(WindowsIdentity.GetCurrent().User,FileSystemRights.FullControl,AccessControlType.Allow)); configSecurity.AddAccessRule(new FileSystemAccessRule(packageSid,FileSystemRights.ReadAndExecute,AccessControlType.Allow)); configSecurity.AddAccessRule(new FileSystemAccessRule(packageSid,FileSystemRights.Write|FileSystemRights.Delete|FileSystemRights.ChangePermissions|FileSystemRights.TakeOwnership,AccessControlType.Deny)); new FileInfo(cargoConfigPath).SetAccessControl(configSecurity); Console.WriteLine("CARGO_CONFIG_ACL_SDDL="+configSecurity.GetSecurityDescriptorSddlForm(AccessControlSections.Access)); variables["MAIA_VERIFIER_CARGO_CONFIG_PATH"]=cargoConfigPath;
            variables["CARGO_TARGET_DIR"] = Path.Combine(target, "cargo-target");
            variables["CARGO_NET_OFFLINE"] = "true";
            variables["CARGO_INCREMENTAL"] = "0";
            if (!useMinimalPath)
            {
                variables["RUSTC"] = Path.Combine(depotRoot, "toolchains", "stable-x86_64-pc-windows-msvc", "bin", "rustc.exe");
                variables["RUSTDOC"] = Path.Combine(depotRoot, "toolchains", "stable-x86_64-pc-windows-msvc", "bin", "rustdoc.exe");
            }
            Console.WriteLine("R3_EXECUTABLE={0}; ARGUMENTS={1}; CWD={2}", childExe, childArgs, target);
            Console.WriteLine("R5L_PATH={0}; minimal_path={1}; bundle_probe={2}; rustup_home_present={3}; rustup_toolchain_present={4}; rustc_present={5}", variables["PATH"], useMinimalPath, runBundleNegative, variables.ContainsKey("RUSTUP_HOME"), variables.ContainsKey("RUSTUP_TOOLCHAIN"), variables.ContainsKey("RUSTC"));
            Console.WriteLine("R5L_RUSTUP_HOME={0}; RUSTUP_TOOLCHAIN={1}; CARGO_HOME={2}", variables.ContainsKey("RUSTUP_HOME") ? variables["RUSTUP_HOME"] : "<absent>", variables.ContainsKey("RUSTUP_TOOLCHAIN") ? variables["RUSTUP_TOOLCHAIN"] : "<absent>", variables["CARGO_HOME"]);
            if (!useMinimalPath) Console.WriteLine("R3_RUSTC={0}; RUSTDOC={1}; CARGO_TARGET_DIR={2}; HOME={3}; TEMP={4}; TMP={5}", variables["RUSTC"], variables["RUSTDOC"], variables["CARGO_TARGET_DIR"], variables["HOME"], variables["TEMP"], variables["TMP"]); else Console.WriteLine("R5L_TOOLCHAIN_ENV=absent; CARGO_TARGET_DIR={0}; TEMP={1}; TMP={2}", variables["CARGO_TARGET_DIR"], variables["TEMP"], variables["TMP"]);
            var envBuilder = new StringBuilder();
            foreach (var item in variables) envBuilder.Append(item.Key).Append('=').Append(item.Value).Append('\0');
            envBuilder.Append('\0');
            string envBlock = envBuilder.ToString();
            IntPtr environment = Marshal.StringToHGlobalUni(envBlock);
            IntPtr bytes = IntPtr.Zero;
            InitializeProcThreadAttributeList(IntPtr.Zero, 1, 0, ref bytes);
            IntPtr list = Marshal.AllocHGlobal(bytes);
            if (!InitializeProcThreadAttributeList(list, 1, 0, ref bytes)) Fail("InitializeProcThreadAttributeList");
            var caps = new SECURITY_CAPABILITIES { AppContainerSid = sid, Capabilities = IntPtr.Zero, CapabilityCount = 0, Reserved = 0 };
            IntPtr pCaps = Marshal.AllocHGlobal(Marshal.SizeOf(typeof(SECURITY_CAPABILITIES)));
            Marshal.StructureToPtr(caps, pCaps, false);
            IntPtr attr = new IntPtr(PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES);
            if (!UpdateProcThreadAttribute(list, 0, attr, pCaps, new IntPtr(Marshal.SizeOf(typeof(SECURITY_CAPABILITIES))), IntPtr.Zero, IntPtr.Zero)) Fail("UpdateProcThreadAttribute SECURITY_CAPABILITIES");
            var si = new STARTUPINFOEX(); si.StartupInfo.cb = (uint)Marshal.SizeOf(typeof(STARTUPINFOEX)); si.AttributeList = list;
            var pi = new PROCESS_INFORMATION();
            bool created = CreateProcess(childExe, command, IntPtr.Zero, IntPtr.Zero, false, EXTENDED_STARTUPINFO_PRESENT | CREATE_SUSPENDED | 0x00000400, environment, target, ref si, out pi);
            if (!created) Fail("CreateProcess AppContainer");
            IntPtr job = CreateJobObject(IntPtr.Zero, null);
            if (job == IntPtr.Zero) Fail("CreateJobObject");
            var limits = new EXTENDED_LIMIT();
            limits.BasicLimitInformation.LimitFlags = 0x2000 | 0x8 | 0x200;
            limits.BasicLimitInformation.ActiveProcessLimit = 64;
            limits.JobMemoryLimit = new UIntPtr(4UL * 1024UL * 1024UL * 1024UL);
            IntPtr pLimits = Marshal.AllocHGlobal(Marshal.SizeOf(typeof(EXTENDED_LIMIT)));
            Marshal.StructureToPtr(limits, pLimits, false);
            if (!SetInformationJobObject(job, 9, pLimits, (uint)Marshal.SizeOf(typeof(EXTENDED_LIMIT)))) Fail("SetInformationJobObject limits");
            var cpu = new CPU_RATE { ControlFlags = 1 | 4, CpuRate = 7500 };
            IntPtr pCpu = Marshal.AllocHGlobal(Marshal.SizeOf(typeof(CPU_RATE)));
            Marshal.StructureToPtr(cpu, pCpu, false);
            if (!SetInformationJobObject(job, 15, pCpu, (uint)Marshal.SizeOf(typeof(CPU_RATE)))) Fail("SetInformationJobObject CPU");
            if (!AssignProcessToJobObject(job, pi.hProcess)) Fail("AssignProcessToJobObject");
            IntPtr childToken; if (!OpenProcessToken(pi.hProcess, TOKEN_QUERY, out childToken)) Fail("OpenProcessToken child");
            int inContainer, returned; if (!GetTokenInformation(childToken, TokenIsAppContainer, out inContainer, 4, out returned)) Fail("GetTokenInformation child");
            int capBytes=0; GetTokenInformation(childToken, TokenCapabilities, IntPtr.Zero, 0, out capBytes);
            if (capBytes < 4) Fail("GetTokenInformation TokenCapabilities size");
            IntPtr capBuffer=Marshal.AllocHGlobal(capBytes);
            if (!GetTokenInformation(childToken, TokenCapabilities, capBuffer, capBytes, out returned)) Fail("GetTokenInformation TokenCapabilities");
            int capCount=Marshal.ReadInt32(capBuffer); var capSids=new List<string>();
            int capOffset=IntPtr.Size == 8 ? 8 : 4; int capStride=IntPtr.Size + 4;
            for(int i=0;i<capCount;i++) { var group=(SID_AND_ATTRIBUTES)Marshal.PtrToStructure(IntPtr.Add(capBuffer,capOffset+i*capStride),typeof(SID_AND_ATTRIBUTES)); capSids.Add(new SecurityIdentifier(group.Sid).Value); }
            bool inJob; if(!IsProcessInJob(pi.hProcess,job,out inJob)) Fail("IsProcessInJob"); if(!inJob) throw new InvalidOperationException("child not in verifier Job Object");
            string networkCaps=String.Join(",",capSids.FindAll(x=>x=="S-1-15-3-1"||x=="S-1-15-3-2"||x=="S-1-15-3-3"));
            Console.WriteLine("pre_resume_appcontainer={0}; appcontainer_sid={1}; capability_count={2}; capability_sids=[{3}]; network_capabilities=[{4}]; job_membership={5}", inContainer, packageSid.Value, capCount, String.Join(",",capSids), networkCaps, inJob);
            Marshal.FreeHGlobal(capBuffer); CloseHandle(childToken);
            if (runRustfmtPair) Console.WriteLine("FMT_A_PID=" + pi.dwProcessId);
            if (ResumeThread(pi.hThread) == 0xffffffff) Fail("ResumeThread");
            uint wait = WaitForSingleObject(pi.hProcess, 900000);
            if (wait != WAIT_OBJECT_0) { if (File.Exists(result)) Console.Error.WriteLine("R3_PARTIAL_RESULT_BEGIN\n" + File.ReadAllText(result) + "\nR3_PARTIAL_RESULT_END"); TerminateProcess(pi.hProcess, 98); WaitForSingleObject(pi.hProcess, INFINITE); Fail("wait native child; wait=" + wait); }
                        uint exit; if (!GetExitCodeProcess(pi.hProcess, out exit)) Fail("GetExitCodeProcess");
            if (runRustfmtPair)
            {
                Console.WriteLine("FMT_A_BASELINE_EXIT=" + exit);
                string src = Path.Combine(target, "workspace", "apps", "local-intelligence-host", "src", "lib.rs");
                string config = Path.Combine(candidate, "rustfmt.toml");
                string argsB = "--check --edition 2024 --config-path \"" + config + "\" \"" + src + "\"";
                string commandB = "\"" + childExe + "\" " + argsB;
                var piB = new PROCESS_INFORMATION();
                var siB = si;
                bool createdB = CreateProcess(childExe, commandB, IntPtr.Zero, IntPtr.Zero, false, EXTENDED_STARTUPINFO_PRESENT | CREATE_SUSPENDED | 0x00000400, environment, target, ref siB, out piB);
                if (!createdB) Fail("CreateProcess AppContainer rustfmt explicit config");
                if (!AssignProcessToJobObject(job, piB.hProcess)) Fail("AssignProcessToJobObject rustfmt explicit config");
                IntPtr tokenB; if (!OpenProcessToken(piB.hProcess, TOKEN_QUERY, out tokenB)) Fail("OpenProcessToken rustfmt explicit config");
                int appB, retB; if (!GetTokenInformation(tokenB, TokenIsAppContainer, out appB, 4, out retB)) Fail("TokenIsAppContainer rustfmt explicit config");
                int capSizeB = 0; GetTokenInformation(tokenB, TokenCapabilities, IntPtr.Zero, 0, out capSizeB);
                if (capSizeB < 4) Fail("TokenCapabilities size rustfmt explicit config");
                IntPtr capBufB = Marshal.AllocHGlobal(capSizeB);
                if (!GetTokenInformation(tokenB, TokenCapabilities, capBufB, capSizeB, out retB)) Fail("TokenCapabilities rustfmt explicit config");
                int capCountB = Marshal.ReadInt32(capBufB);
                bool jobB; if (!IsProcessInJob(piB.hProcess, job, out jobB)) Fail("IsProcessInJob rustfmt explicit config");
                Console.WriteLine("FMT_B_PRE_RESUME_APPCONTAINER=" + appB + "; capability_count=" + capCountB + "; job_membership=" + jobB);
                Marshal.FreeHGlobal(capBufB); CloseHandle(tokenB);
                if (appB != 1 || capCountB != 0 || !jobB) throw new InvalidOperationException("FMT-B containment/token mismatch");
                Console.WriteLine("FMT_B_PID=" + piB.dwProcessId);
                if (ResumeThread(piB.hThread) == 0xffffffff) Fail("ResumeThread rustfmt explicit config");
                uint waitB = WaitForSingleObject(piB.hProcess, 30000);
                if (waitB != WAIT_OBJECT_0) { TerminateProcess(piB.hProcess, 98); WaitForSingleObject(piB.hProcess, INFINITE); Fail("wait rustfmt explicit config; wait=" + waitB); }
                uint exitB; if (!GetExitCodeProcess(piB.hProcess, out exitB)) Fail("GetExitCodeProcess rustfmt explicit config");
                Console.WriteLine("FMT_B_EXPLICIT_CONFIG_EXIT=" + exitB);
                CloseHandle(piB.hThread); CloseHandle(piB.hProcess);
            }
            string builtHello = Path.Combine(target, "hello.exe");
            if (File.Exists(builtHello))
            {
                byte[] hash;
                using (var sha = SHA256.Create()) using (var stream = File.OpenRead(builtHello)) hash = sha.ComputeHash(stream);
                Console.WriteLine("compiled_artifact_bytes={0}; sha256={1}", new FileInfo(builtHello).Length, BitConverter.ToString(hash).Replace("-", ""));
            }
            CloseHandle(pi.hThread); CloseHandle(pi.hProcess); listener.Stop();
            CloseHandle(job); Marshal.FreeHGlobal(pLimits); Marshal.FreeHGlobal(pCpu);
            Console.WriteLine("AppContainer launch={0}; pid={1}; exit={2} (0x{2:X8}); packageSid={3}", created, pi.dwProcessId, exit, packageSid.Value);
            if (File.Exists(result)) Console.WriteLine(File.ReadAllText(result));
            return (int)exit;
        }
        catch (Exception e) { Console.Error.WriteLine(e.ToString()); return 2; }
        finally
        {
            if (nullState != null) RestoreNullDevice(nullState);
            for (int i = aclSnapshots.Count - 1; i >= 0; i--) RestoreDepotAccess(aclSnapshots[i].Path, aclSnapshots[i].Original, aclSnapshots[i].Sddl, packageSidText);
            if (sid != IntPtr.Zero) LocalFree(sid);
            DeleteAppContainerProfile(name);
            try { Directory.Delete(root, true); } catch { Console.Error.WriteLine("Probe cleanup needed: " + root); }
        }
    }

    static void CopyTree(string source, string destination)
    {
        Directory.CreateDirectory(destination);
        foreach (string file in Directory.GetFiles(source))
            File.Copy(file, Path.Combine(destination, Path.GetFileName(file)), true);
        foreach (string directory in Directory.GetDirectories(source))
            CopyTree(directory, Path.Combine(destination, Path.GetFileName(directory)));
    }

    sealed class AclSnapshot { public string Path; public DirectorySecurity Original; public string Sddl; }
    static void GrantTracked(List<AclSnapshot> snapshots, string path, SecurityIdentifier sid, FileSystemRights rights)
    {
        var dir = new DirectoryInfo(path);
        var original = dir.GetAccessControl(AccessControlSections.Access);
        snapshots.Add(new AclSnapshot { Path = path, Original = original, Sddl = original.GetSecurityDescriptorSddlForm(AccessControlSections.Access) });
        Grant(path, sid, rights);
    }
    static void RestoreDepotAccess(string path, DirectorySecurity original, string originalSddl, string sidText) { var dir=new DirectoryInfo(path); dir.SetAccessControl(original); var current=dir.GetAccessControl(AccessControlSections.Access); var explicitRules=current.GetAccessRules(true,false,typeof(SecurityIdentifier)); bool found=false; foreach(AuthorizationRule rule in explicitRules) { var fs=rule as FileSystemAccessRule; if(fs!=null && ((SecurityIdentifier)fs.IdentityReference).Value==sidText) { current.RemoveAccessRuleSpecific(fs); found=true; } } if(found)dir.SetAccessControl(current); var verify=dir.GetAccessControl(AccessControlSections.Access); string actual=verify.GetSecurityDescriptorSddlForm(AccessControlSections.Access); if(actual!=originalSddl) throw new InvalidOperationException("VERIFIER_DEPOT_ACL_RESTORE_MISMATCH; expected="+originalSddl+" actual="+actual); var remaining=verify.GetAccessRules(true,true,typeof(SecurityIdentifier)); foreach(AuthorizationRule rule in remaining) { var fs=rule as FileSystemAccessRule; if(fs!=null && ((SecurityIdentifier)fs.IdentityReference).Value==sidText) throw new InvalidOperationException("VERIFIER_DEPOT_PACKAGE_SID_ACE_REMAINS="+sidText); } Console.WriteLine("VERIFIER_DEPOT_ACL_RESTORE=PASS; package_sid_absent=true"); }
     static void Grant(string path, SecurityIdentifier sid, FileSystemRights rights)
    {
        var directory = new DirectoryInfo(path);
        var security = directory.GetAccessControl(AccessControlSections.Access);
        security.AddAccessRule(new FileSystemAccessRule(sid, rights, InheritanceFlags.ContainerInherit | InheritanceFlags.ObjectInherit, PropagationFlags.None, AccessControlType.Allow));
        directory.SetAccessControl(security);
    }

    static int NulChild(string[] args) { if(args.Length!=2)return 2; string target=Environment.GetEnvironmentVariable("MAIA_APPCONTAINER_CANDIDATE"), workspace=Environment.GetEnvironmentVariable("MAIA_APPCONTAINER_WORKSPACE"), result=args[1]; bool targetWrite=TryWrite(Path.Combine(target,"nul-target-write.tmp")), workspaceWrite=TryWrite(Path.Combine(workspace,"nul-workspace-write.tmp")); bool canonicalRead=CanRead(Environment.GetEnvironmentVariable("MAIA_APPCONTAINER_PROTECTED_FILE")), profileRead=CanRead(@"C:\Users\PS\.codex\memories\MEMORY.md"), hostEvidenceRead=CanRead(@"C:\MAIA\reports\desk_outbox\m0165_r2_prechange_baseline.json"), restoreRecordRead=CanRead(Environment.GetEnvironmentVariable("MAIA_R3_RESTORE_RECORD")); string depotFile=@"C:\MAIA\restricted-verifier-depot\r2-depot-write-"+Guid.NewGuid().ToString("N")+".tmp"; bool depotWrite=TryWrite(depotFile); if(depotWrite){try{File.Delete(depotFile);}catch{}} string vendorRoot=Environment.GetEnvironmentVariable("MAIA_VERIFIER_VENDOR_ROOT"); bool vendorRead=CanRead(Path.Combine(vendorRoot,"ab_glyph","Cargo.toml")); bool vendorWrite=TryWrite(Path.Combine(vendorRoot,"r5v-write-"+Guid.NewGuid().ToString("N")+".tmp")); string configPath=Environment.GetEnvironmentVariable("MAIA_VERIFIER_CARGO_CONFIG_PATH"); bool cargoConfigRead=CanRead(configPath); bool cargoConfigWrite=CanOpenWriteExisting(configPath); int port=int.Parse(Environment.GetEnvironmentVariable("MAIA_APPCONTAINER_LOOPBACK_PORT")); bool network=false; try{using(var client=new TcpClient()){var t=client.ConnectAsync("127.0.0.1",port);network=t.Wait(2000)&&client.Connected;}}catch{} string read=NulAccess(0x80000000,false), write=NulAccess(0x40000000,true); bool deviceAclOpen=NulCanOpen(0x00040000); string evidence="nul_read="+read+"; nul_write="+write+"; target_write="+targetWrite+"; workspace_root_write="+workspaceWrite+"; canonical_repo_read="+canonicalRead+"; unrelated_profile_read="+profileRead+"; protected_host_evidence_read="+hostEvidenceRead+"; restore_record_read="+restoreRecordRead+"; verifier_depot_write="+depotWrite+"; vendor_read="+vendorRead+"; vendor_write="+vendorWrite+"; cargo_config_read="+cargoConfigRead+"; cargo_config_write="+cargoConfigWrite+"; network_connected="+network+"; device_security_write_open="+deviceAclOpen; File.WriteAllText(result,evidence); Console.WriteLine(evidence); if(!read.Contains("io-ok")||!write.Contains("io-ok")||!targetWrite||workspaceWrite||canonicalRead||profileRead||hostEvidenceRead||restoreRecordRead||depotWrite||!vendorRead||vendorWrite||!cargoConfigRead||cargoConfigWrite||network||deviceAclOpen)return 9; return 0; }
    static bool TryWrite(string p){try{using(var f=new FileStream(p,FileMode.CreateNew,FileAccess.Write,FileShare.None)){f.WriteByte(65);} try{File.Delete(p);}catch{} return true;}catch{return false;}}
    static bool CanRead(string p){try{if(String.IsNullOrEmpty(p))return false;using(File.OpenRead(p)){}return true;}catch{return false;}} static bool CanOpenWriteExisting(string p){try{using(File.Open(p,FileMode.Open,FileAccess.Write,FileShare.ReadWrite)){}return true;}catch{return false;}}
    static bool NulCanOpen(uint a){IntPtr h=CreateFileW("\\\\.\\NUL",a,3,IntPtr.Zero,3,0,IntPtr.Zero);if(h==new IntPtr(-1))return false;CloseHandle(h);return true;}
    static string NulAccess(uint access,bool write){IntPtr h=CreateFileW("\\\\.\\NUL",access,3,IntPtr.Zero,3,0,IntPtr.Zero);if(h==new IntPtr(-1))return "open-fail="+Marshal.GetLastWin32Error();try{uint n;bool ok=write?WriteFile(h,new byte[]{65},1,out n,IntPtr.Zero):ReadFile(h,new byte[1],1,out n,IntPtr.Zero);return (ok?"io-ok":"io-fail")+"-count="+n+"-error="+(ok?0:Marshal.GetLastWin32Error());}finally{CloseHandle(h);}}
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern IntPtr CreateFileW(string n,uint a,uint s,IntPtr sa,uint d,uint f,IntPtr t);
    [DllImport("kernel32.dll",SetLastError=true)] static extern bool ReadFile(IntPtr h,byte[] b,uint n,out uint r,IntPtr o);
    [DllImport("kernel32.dll",SetLastError=true)] static extern bool WriteFile(IntPtr h,byte[] b,uint n,out uint w,IntPtr o);
    static int Child(string[] args)
    {
        string candidate = args[1], protectedFile = args[2], result = args[4]; int port = int.Parse(args[3]);
        IntPtr token;
        if (!OpenProcessToken(System.Diagnostics.Process.GetCurrentProcess().Handle, TOKEN_QUERY, out token)) Fail("OpenProcessToken");
        int isContainer, returned; if (!GetTokenInformation(token, TokenIsAppContainer, out isContainer, 4, out returned)) Fail("TokenIsAppContainer");
        IntPtr sidBuffer = Marshal.AllocHGlobal(4096);
        if (!GetTokenInformation(token, TokenAppContainerSid, sidBuffer, 4096, out returned)) Fail("TokenAppContainerSid");
        IntPtr packageSid = Marshal.ReadIntPtr(sidBuffer);
        string sid = packageSid == IntPtr.Zero ? "none" : new SecurityIdentifier(packageSid).Value;
        bool candidateWrite; try { File.WriteAllText(Path.Combine(candidate, "child-write.txt"), "writable"); candidateWrite=true; } catch { candidateWrite=false; }
        bool protectedRead; try { File.ReadAllText(protectedFile); protectedRead=true; } catch (UnauthorizedAccessException) { protectedRead=false; }
        bool networkConnected=false; string netError="none";
        try { using(var client = new TcpClient()) { var task=client.ConnectAsync("127.0.0.1",port); networkConnected=task.Wait(2000) && client.Connected; } } catch(Exception e) { netError=e.GetType().Name+":"+e.Message; }
        File.WriteAllText(result, String.Format("is_appcontainer={0}; package_sid={1}; candidate_write={2}; protected_read={3}; network_connected={4}; network_error={5}", isContainer, sid, candidateWrite, protectedRead, networkConnected, netError));
        return isContainer == 1 && candidateWrite && !protectedRead && !networkConnected ? 0 : 3;
    }
}
