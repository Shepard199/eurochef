import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
public class MessageBoxCallbacksEvidence extends GhidraScript {
 DecompInterface d=new DecompInterface();
 void dec(long x)throws Exception{Function f=getFunctionContaining(toAddr(x));println(String.format("\n=== 0x%08X ===",x));if(f==null){println("NO_FUNCTION");return;}println("FUNCTION="+f.getName()+" ENTRY="+f.getEntryPoint());DecompileResults r=d.decompileFunction(f,60,monitor);if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());else println("ERR="+r.getErrorMessage());}
 public void run()throws Exception{d.openProgram(currentProgram);long[] a={0x004a7c20L,0x004a7cb0L,0x004a7d80L,0x004a7e00L,0x0049c200L,0x004a6700L,0x004a6790L};for(long x:a)dec(x);d.dispose();}
}