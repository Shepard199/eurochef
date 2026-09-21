import ghidra.app.script.GhidraScript;

public class PiranhaDescriptorNameEvidence extends GhidraScript {
    private void cstr(long raw) throws Exception {
        byte[] b=new byte[128];
        int n=currentProgram.getMemory().getBytes(toAddr(raw),b);
        StringBuilder s=new StringBuilder();
        for(int i=0;i<n && b[i]!=0;i++) s.append((char)(b[i]&0xff));
        println(String.format("STR 0x%08X = %s",raw,s.toString()));
    }
    @Override public void run() throws Exception {
        cstr(0x0061B07CL);
        cstr(0x0061B06CL);
        cstr(0x0061B054L);
        cstr(0x0061B090L);
    }
}
