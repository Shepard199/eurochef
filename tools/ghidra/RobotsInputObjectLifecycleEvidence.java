import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
public class RobotsInputObjectLifecycleEvidence extends GhidraScript {
 DecompInterface d=new DecompInterface();
 void dec(long x)throws Exception{Function f=getFunctionContaining(toAddr(x));println(String.format("\n=== 0x%08X ===",x));if(f==null){println("NO_FUNCTION");return;}println(f.getName()+" @ "+f.getEntryPoint());DecompileResults r=d.decompileFunction(f,90,monitor);if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());}
 public void run()throws Exception{d.openProgram(currentProgram);dec(0x004d1610L);dec(0x004d0760L);d.dispose();}
}