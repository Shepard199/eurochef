import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;

public class ProjectileUpdateAndFinalizeEvidence extends GhidraScript {
    private void dump(long raw, DecompInterface d) throws Exception {
        Address a=toAddr(raw); println(String.format("\n=== 0x%08X ===",raw));
        Function f=getFunctionContaining(a);
        if(f!=null){
            println("FUNCTION="+f.getName()+" ENTRY="+f.getEntryPoint());
            DecompileResults r=d.decompileFunction(f,60,monitor);
            if(r.decompileCompleted()&&r.getDecompiledFunction()!=null){ println(r.getDecompiledFunction().getC()); return; }
        }
        println("NO_DECOMPILE");
    }
    @Override public void run() throws Exception {
        DecompInterface d=new DecompInterface(); d.openProgram(currentProgram);
        for(long a:new long[]{0x0041512DL,0x00415260L,0x004152A0L,0x00414EF0L}) dump(a,d);
        d.dispose();
    }
}
