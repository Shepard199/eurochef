import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
public class MessageBoxInputExactEvidence extends GhidraScript {
 DecompInterface d=new DecompInterface();
 public void run()throws Exception{d.openProgram(currentProgram);Function f=getFunctionContaining(toAddr(0x004a6ae0L));println(f.getName()+" "+f.getEntryPoint());DecompileResults r=d.decompileFunction(f,60,monitor);println(r.getDecompiledFunction().getC());d.dispose();}
}