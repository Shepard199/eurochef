import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
public class InputLookupCoreEvidence extends GhidraScript {
 DecompInterface d=new DecompInterface();
 void dec(long x)throws Exception{Function f=getFunctionContaining(toAddr(x));println(String.format("\n=== 0x%08X ===",x));if(f==null){println("NO_FUNCTION");return;}println("FUNCTION="+f.getName()+" ENTRY="+f.getEntryPoint());DecompileResults r=d.decompileFunction(f,60,monitor);if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());else println("ERR="+r.getErrorMessage());}
 public void run()throws Exception{d.openProgram(currentProgram);dec(0x00508508L);d.dispose();}
}