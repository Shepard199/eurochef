import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;

public class StandardMonsterOverrideFacts extends GhidraScript {
    private long ptr(long a) throws Exception { return Integer.toUnsignedLong(getInt(toAddr(a))); }
    private void row(String name,long vt) throws Exception {
        println(String.format("%s vt=%08X ctor=%08X update=%08X first=%08X builder=%08X",
            name,vt,ptr(vt+0x08),ptr(vt+0x34),ptr(vt+0x100),ptr(vt+0x108)));
    }
    private void f(long a,String name) throws Exception {
        int bits=getInt(toAddr(a));
        println(String.format("%s @%08X bits=%08X value=%.9g",name,a,bits,Float.intBitsToFloat(bits)));
    }
    private void dc(long a,DecompInterface d) throws Exception {
        Function fn=getFunctionContaining(toAddr(a));
        println(String.format("\n=== %08X %s ===",a,fn==null?"<missing>":fn.getName()));
        if(fn==null)return;
        DecompileResults r=d.decompileFunction(fn,90,monitor);
        if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
    }
    @Override public void run() throws Exception {
        row("EB13",0x005E6338L); row("EB14",0x005E61D0L); row("EW10",0x005E6068L);
        f(0x005DD730L,"DAT_005dd730");
        f(0x005DD72CL,"DAT_005dd72c");
        f(0x00620034L,"DAT_00620034");
        f(0x005E6E20L,"DAT_005e6e20");
        DecompInterface d=new DecompInterface(); d.openProgram(currentProgram);
        long eb13first=ptr(0x005E6338L+0x100), eb14first=ptr(0x005E61D0L+0x100), ew10first=ptr(0x005E6068L+0x100);
        dc(eb13first,d); dc(eb14first,d); dc(ew10first,d);
        d.dispose();
    }
}