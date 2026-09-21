import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;

public class MineProjectileBehaviorEvidence extends GhidraScript {
    private void dump(long raw, DecompInterface d) throws Exception {
        Address a=toAddr(raw); println(String.format("\n=== 0x%08X ===", raw));
        Function f=getFunctionContaining(a);
        if(f==null){ println("NO_FUNCTION"); return; }
        println("FUNCTION="+f.getName()+" ENTRY="+f.getEntryPoint());
        DecompileResults r=d.decompileFunction(f,60,monitor);
        if(r.decompileCompleted()&&r.getDecompiledFunction()!=null) println(r.getDecompiledFunction().getC());
    }
    @Override public void run() throws Exception {
        DecompInterface d=new DecompInterface(); d.openProgram(currentProgram);
        for(long a:new long[]{0x00414810L,0x00415250L,0x00404E90L,0x004148B0L,0x004149A0L}) dump(a,d);
        d.dispose();
    }
}
