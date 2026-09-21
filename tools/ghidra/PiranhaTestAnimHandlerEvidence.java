import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;

public class PiranhaTestAnimHandlerEvidence extends GhidraScript {
    private long ptr(long a) throws Exception { return Integer.toUnsignedLong(getInt(toAddr(a))); }
    private void dec(long raw, DecompInterface d) throws Exception {
        Function f = getFunctionAt(toAddr(raw));
        if (f == null) f = getFunctionContaining(toAddr(raw));
        println(String.format("\n=== 0x%08X %s ===", raw, f == null ? "<missing>" : f.getName()));
        if (f == null) return;
        DecompileResults r = d.decompileFunction(f, 60, monitor);
        if (r.decompileCompleted() && r.getDecompiledFunction() != null) println(r.getDecompiledFunction().getC());
    }
    private void vt(long base, String name, DecompInterface d) throws Exception {
        println("\n=== " + name + String.format(" VTABLE 0x%08X ===", base));
        int[] slots={0x08,0x34,0x50,0xC8,0x100,0x104,0x108,0x10C,0x110,0x114,0x118,0x11C,0x120,0x124,0x128,0x12C,0x130,0x138,0x13C,0x140};
        for(int s:slots) println(String.format("+%03X -> %08X",s,ptr(base+s)));
        dec(ptr(base+0x08),d);
        dec(ptr(base+0x34),d);
        dec(ptr(base+0x108),d);
    }
    @Override public void run() throws Exception {
        DecompInterface d=new DecompInterface(); d.openProgram(currentProgram);
        dec(0x0045AB60L,d);
        dec(0x0045ACC0L,d);
        vt(0x005E6A40L,"candidate_EM07",d);
        vt(0x005E6BA8L,"TestAnim",d);
        d.dispose();
    }
}
