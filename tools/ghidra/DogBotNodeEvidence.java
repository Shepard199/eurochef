import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;

public class DogBotNodeEvidence extends GhidraScript {
    private void dump(long raw, DecompInterface d) throws Exception {
        Function f=getFunctionContaining(toAddr(raw));
        println(String.format("\n=== 0x%08X ===",raw));
        if(f==null){println("NO FUNCTION");return;}
        println("FUNCTION="+f.getName()+" ENTRY="+f.getEntryPoint());
        DecompileResults r=d.decompileFunction(f,60,monitor);
        if(r.decompileCompleted()&&r.getDecompiledFunction()!=null) println(r.getDecompiledFunction().getC());
    }
    @Override public void run() throws Exception {
        DecompInterface d=new DecompInterface(); d.openProgram(currentProgram);
        for(long t:new long[]{0x0046D9E0L,0x0046A850L,0x0046CB30L,0x0044EE20L,0x00469E20L,0x00460870L}) dump(t,d);
        d.dispose();
    }
}
