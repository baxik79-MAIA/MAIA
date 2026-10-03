using System;
using System.Runtime.InteropServices;

public static class M0165NullStdinCapabilityAcl
{
    internal const string CapabilityName = "maia.evolution.tier1.null.stdin";
    internal const uint FileGenericRead = 0x00120089;
    const uint ReadControl = 0x00020000;
    const uint WriteDac = 0x00040000;
    const uint DaclSecurityInformation = 0x00000004;
    const int SeFileObject = 1;
    const uint GrantAccess = 1;
    const uint RevokeAccess = 4;

    [StructLayout(LayoutKind.Sequential)] struct UnicodeString
    {
        public ushort Length, MaximumLength;
        public IntPtr Buffer;
    }
    [StructLayout(LayoutKind.Sequential)] struct ObjectAttributes
    {
        public uint Length;
        public IntPtr RootDirectory, ObjectName;
        public uint Attributes;
        public IntPtr SecurityDescriptor, SecurityQualityOfService;
    }
    [StructLayout(LayoutKind.Sequential)] struct IoStatusBlock
    {
        public IntPtr Status;
        public UIntPtr Information;
    }
    [StructLayout(LayoutKind.Sequential)] struct Trustee
    {
        public IntPtr MultipleTrustee;
        public int MultipleTrusteeOperation, TrusteeForm, TrusteeType;
        public IntPtr Name;
    }
    [StructLayout(LayoutKind.Sequential)] struct ExplicitAccess
    {
        public uint Permissions;
        public uint AccessMode;
        public uint Inheritance;
        public Trustee Trustee;
    }
    sealed class DeviceState : IDisposable
    {
        public IntPtr Handle, Dacl, Descriptor;
        public void Dispose()
        {
            if (Descriptor != IntPtr.Zero) LocalFree(Descriptor);
            if (Handle != IntPtr.Zero) CloseHandle(Handle);
            Descriptor = Handle = Dacl = IntPtr.Zero;
        }
    }

    [DllImport("api-ms-win-security-base-l1-2-2.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    static extern bool DeriveCapabilitySidsFromName(string name, out IntPtr groupSids,
        out uint groupCount, out IntPtr capabilitySids, out uint capabilityCount);
    [DllImport("ntdll.dll")] static extern int NtOpenFile(out IntPtr handle, uint access,
        ref ObjectAttributes attributes, out IoStatusBlock io, uint share, uint options);
    [DllImport("advapi32.dll", SetLastError = true)] static extern uint GetSecurityInfo(IntPtr handle,
        int objectType, uint securityInfo, IntPtr owner, IntPtr group, out IntPtr dacl,
        IntPtr sacl, out IntPtr descriptor);
    [DllImport("advapi32.dll", SetLastError = true)] static extern uint SetSecurityInfo(IntPtr handle,
        int objectType, uint securityInfo, IntPtr owner, IntPtr group, IntPtr dacl, IntPtr sacl);
    [DllImport("advapi32.dll", SetLastError = true)] static extern uint SetEntriesInAclW(uint count,
        ref ExplicitAccess entries, IntPtr oldAcl, out IntPtr newAcl);
    [DllImport("advapi32.dll", SetLastError = true)] static extern bool GetAce(IntPtr acl,
        uint index, out IntPtr ace);
    [DllImport("advapi32.dll", SetLastError = true)] static extern bool EqualSid(IntPtr left, IntPtr right);
    [DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    static extern bool ConvertSecurityDescriptorToStringSecurityDescriptorW(IntPtr descriptor,
        uint revision, uint information, out IntPtr text, out uint length);
    [DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    static extern bool ConvertSidToStringSidW(IntPtr sid, out IntPtr text);
    [DllImport("kernel32.dll", SetLastError = true)] static extern bool CloseHandle(IntPtr handle);
    [DllImport("kernel32.dll")] static extern IntPtr LocalFree(IntPtr memory);

    static IntPtr DeriveSid()
    {
        IntPtr groups = IntPtr.Zero, capabilities = IntPtr.Zero;
        uint groupCount = 0, capabilityCount = 0;
        if (!DeriveCapabilitySidsFromName(CapabilityName, out groups, out groupCount,
                out capabilities, out capabilityCount) || capabilities == IntPtr.Zero || capabilityCount != 1)
            throw new InvalidOperationException("DeriveCapabilitySidsFromName failed or returned unexpected SID count; Win32=" + Marshal.GetLastWin32Error());
        IntPtr sid = Marshal.ReadIntPtr(capabilities);
        if (sid == IntPtr.Zero) throw new InvalidOperationException("Capability SID is null");
        _derivedArrays.Add(new Tuple<IntPtr, uint, IntPtr, uint>(groups, groupCount, capabilities, capabilityCount));
        return sid;
    }

    static readonly System.Collections.Generic.List<Tuple<IntPtr, uint, IntPtr, uint>> _derivedArrays =
        new System.Collections.Generic.List<Tuple<IntPtr, uint, IntPtr, uint>>();

    static void FreeDerivedSids()
    {
        foreach (var item in _derivedArrays)
        {
            for (uint i = 0; i < item.Item2; i++) LocalFree(Marshal.ReadIntPtr(item.Item1, checked((int)i * IntPtr.Size)));
            if (item.Item1 != IntPtr.Zero) LocalFree(item.Item1);
            for (uint i = 0; i < item.Item4; i++) LocalFree(Marshal.ReadIntPtr(item.Item3, checked((int)i * IntPtr.Size)));
            if (item.Item3 != IntPtr.Zero) LocalFree(item.Item3);
        }
        _derivedArrays.Clear();
    }

    static DeviceState OpenDevice(bool writable)
    {
        var name = Marshal.StringToHGlobalUni("\\Device\\Null");
        var unicode = new UnicodeString { Length = 24, MaximumLength = 26, Buffer = name };
        var pUnicode = Marshal.AllocHGlobal(Marshal.SizeOf(typeof(UnicodeString)));
        Marshal.StructureToPtr(unicode, pUnicode, false);
        var attributes = new ObjectAttributes { Length = (uint)Marshal.SizeOf(typeof(ObjectAttributes)),
            ObjectName = pUnicode, Attributes = 0x40 };
        IoStatusBlock io;
        IntPtr handle;
        int status = NtOpenFile(out handle, ReadControl | (writable ? WriteDac : 0), ref attributes,
            out io, 3, 0);
        Marshal.FreeHGlobal(pUnicode);
        Marshal.FreeHGlobal(name);
        if (status < 0 || handle == IntPtr.Zero)
            throw new InvalidOperationException("NtOpenFile(\\Device\\Null, READ_CONTROL" +
                (writable ? "|WRITE_DAC" : "") + ") NTSTATUS=0x" + unchecked((uint)status).ToString("X8"));
        var result = new DeviceState { Handle = handle };
        uint query = GetSecurityInfo(handle, SeFileObject, DaclSecurityInformation, IntPtr.Zero,
            IntPtr.Zero, out result.Dacl, IntPtr.Zero, out result.Descriptor);
        if (query != 0 || result.Descriptor == IntPtr.Zero || result.Dacl == IntPtr.Zero)
        {
            result.Dispose();
            throw new InvalidOperationException("GetSecurityInfo(\\Device\\Null DACL) Win32=" + query);
        }
        return result;
    }

    static string Sddl(IntPtr descriptor)
    {
        IntPtr text; uint length;
        if (!ConvertSecurityDescriptorToStringSecurityDescriptorW(descriptor, 1,
                DaclSecurityInformation, out text, out length))
            throw new InvalidOperationException("ConvertSecurityDescriptorToStringSecurityDescriptorW Win32=" + Marshal.GetLastWin32Error());
        try { return Marshal.PtrToStringUni(text); }
        finally { LocalFree(text); }
    }

    // Returns the number of ACEs for this SID and whether exactly one has the required mask.
    static Tuple<int, bool> CapabilityAceState(IntPtr dacl, IntPtr sid)
    {
        int count = 0; bool exact = false;
        ushort aceCount = unchecked((ushort)Marshal.ReadInt16(dacl, 4));
        for (uint i = 0; i < aceCount; i++)
        {
            IntPtr ace;
            if (!GetAce(dacl, i, out ace) || ace == IntPtr.Zero) throw new InvalidOperationException("GetAce failed; Win32=" + Marshal.GetLastWin32Error());
            byte aceType = Marshal.ReadByte(ace, 0), aceFlags = Marshal.ReadByte(ace, 1);
            if (aceType != 0 || !EqualSid(IntPtr.Add(ace, 8), sid)) continue;
            count++;
            exact = aceFlags == 0 && unchecked((uint)Marshal.ReadInt32(ace, 4)) == FileGenericRead;
            if (!exact) break;
        }
        return Tuple.Create(count, exact);
    }

    static ExplicitAccess Entry(IntPtr sid, uint mode)
    {
        return new ExplicitAccess { Permissions = mode == RevokeAccess ? 0 : FileGenericRead,
            AccessMode = mode, Inheritance = 0,
            Trustee = new Trustee { MultipleTrustee = IntPtr.Zero, MultipleTrusteeOperation = 0,
                TrusteeForm = 0, TrusteeType = 0, Name = sid } };
    }

    public static string Probe()
    {
        try
        {
            IntPtr sid = DeriveSid();
            using (DeviceState state = OpenDevice(false))
            {
                var ace = CapabilityAceState(state.Dacl, sid);
                return "capability_name=" + CapabilityName + Environment.NewLine +
                    "capability_sid=" + SidString(sid) + Environment.NewLine +
                    "ace_count_for_sid=" + ace.Item1 + Environment.NewLine +
                    "exact_read_only_noninherited_ace=" + (ace.Item1 == 1 && ace.Item2) + Environment.NewLine +
                    "dacl_sddl=" + Sddl(state.Descriptor);
            }
        }
        finally { FreeDerivedSids(); }
    }

    public static string Apply(bool remove)
    {
        try
        {
            IntPtr sid = DeriveSid();
            string before;
            using (DeviceState state = OpenDevice(true))
            {
                before = Sddl(state.Descriptor);
                var ace = CapabilityAceState(state.Dacl, sid);
                if (!remove && ace.Item1 == 1 && ace.Item2)
                    return "result=already_installed" + Environment.NewLine + "capability_sid=" + SidString(sid) +
                        Environment.NewLine + "dacl_before=" + before + Environment.NewLine + "dacl_after=" + before;
                if (ace.Item1 > 0 && (ace.Item1 != 1 || !remove || !ace.Item2))
                    throw new InvalidOperationException("Existing capability SID ACE does not match the expected exact read-only ACE; refusing update");
                if (remove && ace.Item1 == 0)
                    return "result=already_absent" + Environment.NewLine + "capability_sid=" + SidString(sid) +
                        Environment.NewLine + "dacl_before=" + before + Environment.NewLine + "dacl_after=" + before;

                ExplicitAccess entry = Entry(sid, remove ? RevokeAccess : GrantAccess);
                IntPtr newAcl;
                uint aclStatus = SetEntriesInAclW(1, ref entry, state.Dacl, out newAcl);
                if (aclStatus != 0 || newAcl == IntPtr.Zero)
                    throw new InvalidOperationException("SetEntriesInAclW failed; Win32=" + aclStatus);
                try
                {
                    uint setStatus = SetSecurityInfo(state.Handle, SeFileObject, DaclSecurityInformation,
                        IntPtr.Zero, IntPtr.Zero, newAcl, IntPtr.Zero);
                    if (setStatus != 0) throw new InvalidOperationException("SetSecurityInfo(\\Device\\Null DACL) failed; Win32=" + setStatus);
                }
                finally { LocalFree(newAcl); }
            }
            using (DeviceState verify = OpenDevice(false))
            {
                string after = Sddl(verify.Descriptor);
                var ace = CapabilityAceState(verify.Dacl, sid);
                if (remove ? ace.Item1 != 0 : (ace.Item1 != 1 || !ace.Item2))
                    throw new InvalidOperationException("Post-update DACL verification failed; ACE count=" + ace.Item1);
                return "result=" + (remove ? "removed" : "installed") + Environment.NewLine +
                    "capability_sid=" + SidString(sid) + Environment.NewLine +
                    "dacl_before=" + before + Environment.NewLine + "dacl_after=" + after +
                    Environment.NewLine + "exact_ace_verified=true";
            }
        }
        finally { FreeDerivedSids(); }
    }

    static string SidString(IntPtr sid)
    {
        IntPtr text;
        if (!ConvertSidToStringSidW(sid, out text))
            throw new InvalidOperationException("ConvertSidToStringSidW failed; Win32=" + Marshal.GetLastWin32Error());
        try { return Marshal.PtrToStringUni(text); }
        finally { LocalFree(text); }
    }
}
