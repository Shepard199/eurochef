import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;

public class TriggerLinkGetterEvidence extends GhidraScript {
    private Function ensure(long raw)throws Exception{
        Address a=toAddr(raw);disassemble(a);Function f=getFunctionAt(a);
        if(f==null){try{f=createFunction(a,null);}catch(Exception ignored){}}
        if(f==null)f=getFunctionContaining(a);return f;
    }
    private void dump(long raw,DecompInterface d)throws Exception{
        Function f=ensure(raw);println(String.format("\n=== 0x%08X %s ===",raw,f==null?"<missing>":f.getName()));
        if(f==null)return;DecompileResults r=d.decompileFunction(f,90,monitor);
        if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
    }
    @Override public void run()throws Exception{
        DecompInterface d=new DecompInterface();d.openProgram(currentProgram);
        dump(0x0044CD10L,d);
        dump(0x0044CCA0L,d);
        d.dispose();
    }
}
