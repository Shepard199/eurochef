import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
import ghidra.program.model.address.Address;
public class RobotsInputActionMethodsEvidence extends GhidraScript {
 DecompInterface d=new DecompInterface();
 void createDec(long x)throws Exception{
   Address a=toAddr(x);disassemble(a);Function f=getFunctionAt(a);
   if(f==null){try{f=createFunction(a,null);}catch(Exception e){println("CREATE_FAIL "+Long.toHexString(x)+" "+e.getMessage());}}
   println(String.format("\n=== 0x%08X ===",x));
   if(f==null){println("NO_FUNCTION");return;}
   println(f.getName()+" @ "+f.getEntryPoint()+" BODY="+f.getBody());
   DecompileResults r=d.decompileFunction(f,60,monitor);
   if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
 }
 public void run()throws Exception{
   d.openProgram(currentProgram);
   long[] a={0x004d2130L,0x004d2150L,0x004d2170L,0x004d2190L,0x004d21b0L,0x004d21e0L};
   for(long x:a)createDec(x);
   d.dispose();
 }
}