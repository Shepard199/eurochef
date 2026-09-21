import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
public class Ef01PhysicsOverridesEvidence extends GhidraScript {
 private void dc(long a,DecompInterface d)throws Exception{Function f=getFunctionContaining(toAddr(a));println(String.format("\n===%08X %s===",a,f==null?"<missing>":f.getName()));if(f!=null){DecompileResults r=d.decompileFunction(f,90,monitor);if(r.decompileCompleted())println(r.getDecompiledFunction().getC());}}
 @Override public void run() throws Exception{DecompInterface d=new DecompInterface();d.openProgram(currentProgram);dc(0x00453640L,d);dc(0x00455DD0L,d);dc(0x00463B80L,d);d.dispose();}
}