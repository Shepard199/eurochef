import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
public class Eq02BaseCtorEvidence extends GhidraScript {
 @Override public void run() throws Exception{DecompInterface d=new DecompInterface();d.openProgram(currentProgram);Function f=getFunctionContaining(toAddr(0x004511E0L));if(f!=null){DecompileResults r=d.decompileFunction(f,90,monitor);if(r.decompileCompleted())println(r.getDecompiledFunction().getC());}d.dispose();}
}