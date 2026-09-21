import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
import java.util.LinkedHashSet;
import java.util.Set;

public class BehaviorNodeVtableEvidence extends GhidraScript {
    private long ptr(long a) throws Exception { return Integer.toUnsignedLong(getInt(toAddr(a))); }
    private void vt(String name,long v, DecompInterface d) throws Exception {
        println("\n=== "+name+String.format(" VTABLE 0x%08X ===",v));
        Set<Long> funcs=new LinkedHashSet<>();
        for(int off=0;off<=0x40;off+=4){ long p=ptr(v+off); println(String.format("+%02X -> %08X",off,p)); if(p>=0x00400000L&&p<0x00590000L) funcs.add(p); }
        for(long p:funcs){ Function f=getFunctionContaining(toAddr(p)); if(f==null) continue; println(String.format("\n--- %08X %s ---",p,f.getName())); DecompileResults r=d.decompileFunction(f,30,monitor); if(r.decompileCompleted()&&r.getDecompiledFunction()!=null) println(r.getDecompiledFunction().getC()); }
    }
    private long ctorVt(long c, DecompInterface d) throws Exception {
        Function f=getFunctionContaining(toAddr(c)); println(String.format("\n=== CTOR %08X ===",c)); if(f!=null){DecompileResults r=d.decompileFunction(f,30,monitor); if(r.decompileCompleted()&&r.getDecompiledFunction()!=null) println(r.getDecompiledFunction().getC());} return 0;
    }
    private void diffVt(String name,long a,long b,int bytes) throws Exception {
        println("\n=== "+name+String.format(" DIFF 0x%08X -> 0x%08X (%d bytes) ===",a,b,bytes));
        for(int off=0;off<bytes;off+=4){ long x=ptr(a+off), y=ptr(b+off); if(x!=y) println(String.format("+%03X: %08X -> %08X",off,x,y)); }
    }
    @Override public void run() throws Exception {
        DecompInterface d=new DecompInterface();d.openProgram(currentProgram);
        ctorVt(0x0046A390L,d); ctorVt(0x004508A0L,d); ctorVt(0x0046B4A0L,d);
        ctorVt(0x0046B210L,d); ctorVt(0x0046BC10L,d);
        ctorVt(0x00458BA0L,d); ctorVt(0x004590A0L,d); ctorVt(0x0046BDB0L,d); ctorVt(0x00458C50L,d);
        diffVt("NpcVsFender",0x005E7048L,0x005E71B8L,0x170);
        vt("Dog6D9E0",0x005E74E8L,d); vt("Dog6A850",0x005E6F68L,d); vt("Dog6CB30",0x005E7428L,d); vt("Dog44EE20",0x005E1FBCL,d); vt("Dog69E20",0x005E6E78L,d);
        vt("Npc58BA0",0x005E2508L,d); vt("Npc590A0",0x005E25D0L,d); vt("Npc6BDB0",0x005E7368L,d); vt("Npc58C50",0x005E2548L,d);
        d.dispose();
    }
}
