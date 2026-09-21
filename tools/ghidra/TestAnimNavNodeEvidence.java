import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;

public class TestAnimNavNodeEvidence extends GhidraScript {
    private long ptr(long a) throws Exception { return Integer.toUnsignedLong(getInt(toAddr(a))); }
    private void dec(long raw, DecompInterface d) throws Exception {
        disassemble(toAddr(raw));
        Function f=getFunctionAt(toAddr(raw));
        if(f==null){try{f=createFunction(toAddr(raw),"evidence_"+Long.toHexString(raw));}catch(Exception ignored){}}
        if(f==null)f=getFunctionContaining(toAddr(raw));
        println(String.format("\n=== 0x%08X %s ===",raw,f==null?"<missing>":f.getName()));
        if(f==null)return;
        DecompileResults r=d.decompileFunction(f,60,monitor);
        if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
    }
    @Override public void run() throws Exception {
        DecompInterface d=new DecompInterface(); d.openProgram(currentProgram);
        dec(0x0046D840L,d);
        for(long v=0x005E6F00L;v<=0x005E7600L;v+=4){
            if(ptr(v+0x38)==0x0046D840L){ println(String.format("ctor-vtable-candidate 0x%08X",v)); }
        }
        d.dispose();
    }
}
